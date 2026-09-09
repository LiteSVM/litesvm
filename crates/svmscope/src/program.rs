//! IDL-driven construction, signing, submission, and capture of program calls.

use {
    crate::{
        error::{Error, Result},
        idl_encode,
        idl_model::{AccountNode, DiscField, IdlModel, IxDef},
        CapturedTransaction, Scope,
    },
    serde_json::{Map, Value},
    sha2::{Digest, Sha256},
    solana_address::Address,
    solana_instruction::{AccountMeta, Instruction},
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
    std::{
        collections::{HashMap, HashSet},
        str::FromStr,
    },
};

/// An IDL-backed client for one deployed Solana program.
pub struct ProgramClient<'a> {
    scope: &'a Scope,
    program_id: Address,
    idl: Value,
}

/// A fluent request to invoke one IDL method.
pub struct MethodBuilder<'a> {
    scope: &'a Scope,
    program_id: Address,
    idl: Value,
    model: IdlModel,
    method: String,
    args: Map<String, Value>,
    accounts: HashMap<String, Address>,
    payer: Option<&'a dyn Signer>,
    signers: Vec<&'a dyn Signer>,
}

impl Scope {
    /// Build a program client from a caller-supplied IDL, which is the normal
    /// path for a program deployed only to a local validator.
    pub fn program_with_idl(&self, program_id: Address, idl: Value) -> ProgramClient<'_> {
        // The IDL you build with is the IDL everything else on this scope
        // should decode with — register it so by-signature APIs name the
        // program too, without a second call.
        self.add_idl(program_id.to_string(), idl.clone());
        ProgramClient {
            scope: self,
            program_id,
            idl,
        }
    }

    /// Build a program client from an IDL published on-chain.
    pub fn program(&self, program_id: Address) -> Result<ProgramClient<'_>> {
        let idl = self
            .program_idl(&program_id.to_string())?
            .ok_or_else(|| Error::NoIdl(program_id.to_string()))?;
        Ok(self.program_with_idl(program_id, idl))
    }
}

impl<'a> ProgramClient<'a> {
    /// Select one instruction by its exact IDL method name.
    pub fn method(&self, name: impl Into<String>) -> Result<MethodBuilder<'a>> {
        let method = name.into();
        let model = IdlModel::parse(&self.idl);
        if model.instruction(&method).is_none() {
            return Err(Error::MethodNotFound {
                program: self.program_id.to_string(),
                method,
            });
        }
        Ok(MethodBuilder {
            scope: self.scope,
            program_id: self.program_id,
            idl: self.idl.clone(),
            model,
            method,
            args: Map::new(),
            accounts: HashMap::new(),
            payer: None,
            signers: Vec::new(),
        })
    }

    /// The program this client targets.
    pub fn program_id(&self) -> Address {
        self.program_id
    }
}

impl<'a> MethodBuilder<'a> {
    /// Set the fee payer, which is always included as a transaction signer.
    pub fn payer(mut self, payer: &'a dyn Signer) -> Self {
        self.payer = Some(payer);
        self
    }

    /// Add an extra transaction signer.
    pub fn signer(mut self, signer: &'a dyn Signer) -> Self {
        self.push_signer(signer);
        self
    }

    /// Supply one instruction account by its IDL name.
    pub fn account(mut self, name: impl Into<String>, address: Address) -> Self {
        self.accounts.insert(name.into(), address);
        self
    }

    /// Supply an instruction account and register the same object as its signer.
    pub fn account_signer(mut self, name: impl Into<String>, signer: &'a dyn Signer) -> Self {
        self.accounts.insert(name.into(), signer.pubkey());
        self.push_signer(signer);
        self
    }

    /// Supply one JSON-compatible argument value.
    pub fn arg(mut self, name: impl Into<String>, value: impl Into<Value>) -> Self {
        self.args.insert(name.into(), value.into());
        self
    }

    /// Supply several arguments as one JSON object.
    pub fn args(mut self, args: Value) -> Result<Self> {
        let object = args
            .as_object()
            .ok_or_else(|| Error::InvalidSpec("method arguments must be a JSON object".into()))?;
        self.args.extend(object.clone());
        Ok(self)
    }

    /// The IDL method this builder invokes.
    pub fn method_name(&self) -> &str {
        &self.method
    }

    /// The program this builder targets.
    pub fn program_id(&self) -> Address {
        self.program_id
    }

    /// Validate, encode, and assemble the Solana instruction.
    pub fn instruction(&self) -> Result<Instruction> {
        self.validate()?;
        let instruction = self.idl_instruction();
        let mut data = instruction_discriminator(instruction, &self.method)?;

        let mut account_metas = Vec::new();
        collect_account_specs(
            &instruction.accounts,
            "",
            &mut |full_name, leaf_name, spec| {
                let address = self.resolve_account(full_name, leaf_name, spec)?;
                let writable = spec.writable();
                let signer = spec.signer();
                account_metas.push(if writable {
                    AccountMeta::new(address, signer)
                } else {
                    AccountMeta::new_readonly(address, signer)
                });
                Ok(())
            },
        )?;
        idl_encode::encode_arguments(&self.model, instruction, &self.args, &mut data)?;

        Ok(Instruction {
            program_id: self.program_id,
            accounts: account_metas,
            data,
        })
    }

    /// Build and sign a legacy transaction using the validator's latest blockhash.
    pub fn transaction(&self) -> Result<VersionedTransaction> {
        self.validate()?;
        let payer = self.payer.ok_or_else(|| Error::MissingPayer {
            method: self.method.clone(),
        })?;
        let instruction = self.instruction()?;
        let blockhash = self
            .scope
            .client()
            .get_latest_blockhash()
            .map_err(Error::rpc)?;
        let message =
            Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
        let mut signers: Vec<&dyn Signer> = vec![payer];
        for signer in &self.signers {
            if !signers
                .iter()
                .any(|existing| existing.pubkey() == signer.pubkey())
            {
                signers.push(*signer);
            }
        }
        VersionedTransaction::try_new(VersionedMessage::Legacy(message), &signers)
            .map_err(|error| Error::TransactionBuild(error.to_string()))
    }

    /// Build, sign, submit, confirm, and return a replay retaining the exact
    /// pre-transaction account world.
    pub fn send_and_capture(self) -> Result<CapturedTransaction> {
        let program_id = self.program_id;
        let idl = self.idl.clone();
        let transaction = self.transaction()?;
        let mut captured = self.scope.send_and_capture(transaction)?;
        captured.replay.add_idl(program_id.to_string(), idl);
        Ok(captured)
    }

    fn validate(&self) -> Result<()> {
        let payer = self.payer.ok_or_else(|| Error::MissingPayer {
            method: self.method.clone(),
        })?;
        let instruction = self.idl_instruction();
        collect_account_specs(
            &instruction.accounts,
            "",
            &mut |full_name, leaf_name, spec| {
                let address = self.resolve_account(full_name, leaf_name, spec)?;
                if spec.signer()
                    && payer.pubkey() != address
                    && !self.signers.iter().any(|signer| signer.pubkey() == address)
                {
                    return Err(Error::MissingSigner {
                        account: full_name.to_string(),
                        address: address.to_string(),
                    });
                }
                Ok(())
            },
        )?;

        let known: HashSet<&str> = instruction
            .args
            .iter()
            .filter_map(|argument| argument.name.as_deref())
            .collect();
        for argument in &instruction.args {
            let name = argument
                .name
                .as_deref()
                .ok_or_else(|| Error::InvalidSpec("IDL argument is missing its name".into()))?;
            if !self.args.contains_key(name) {
                return Err(Error::MissingArgument {
                    method: self.method.clone(),
                    argument: name.to_string(),
                });
            }
        }
        if let Some(argument) = self.args.keys().find(|name| !known.contains(name.as_str())) {
            return Err(Error::UnknownArgument {
                method: self.method.clone(),
                argument: argument.clone(),
            });
        }
        Ok(())
    }

    fn idl_instruction(&self) -> &IxDef {
        self.model
            .instruction(&self.method)
            .expect("method existence checked when the builder was created")
    }

    fn resolve_account(
        &self,
        full_name: &str,
        leaf_name: &str,
        spec: &AccountNode,
    ) -> Result<Address> {
        // Exact dotted name wins.
        if let Some(address) = self.accounts.get(full_name) {
            return Ok(*address);
        }
        // Bare leaf-name fallback — but only when the leaf is unique in the
        // instruction. Two nested groups can share a leaf (`a.vault`, `b.vault`);
        // binding both from one `.account("vault", …)` would silently send the
        // wrong account, so an ambiguous leaf is an error, not a lucky guess.
        if let Some(address) = self.accounts.get(leaf_name) {
            let sharing = self.accounts_sharing_leaf(leaf_name);
            if sharing.len() > 1 {
                return Err(Error::AmbiguousField {
                    field: leaf_name.to_string(),
                    candidates: sharing,
                });
            }
            return Ok(*address);
        }
        // A fixed address pinned by the IDL (e.g. system_program).
        if let Some(address) = spec.address.as_deref() {
            return Address::from_str(address)
                .map_err(|_| Error::InvalidAddress(address.to_string()));
        }
        Err(Error::MissingInstructionAccount {
            method: self.method.clone(),
            account: full_name.to_string(),
        })
    }

    /// The fully qualified names of every instruction account whose leaf segment
    /// equals `leaf`. Used to reject an ambiguous bare-leaf account binding.
    fn accounts_sharing_leaf(&self, leaf: &str) -> Vec<String> {
        let instruction = self.idl_instruction();
        let mut names = Vec::new();
        let _ = collect_account_specs(
            &instruction.accounts,
            "",
            &mut |full_name, leaf_name, _spec| {
                if leaf_name == leaf {
                    names.push(full_name.to_string());
                }
                Ok(())
            },
        );
        names
    }

    fn push_signer(&mut self, signer: &'a dyn Signer) {
        if !self
            .signers
            .iter()
            .any(|existing| existing.pubkey() == signer.pubkey())
        {
            self.signers.push(signer);
        }
    }
}

fn collect_account_specs(
    specs: &[AccountNode],
    prefix: &str,
    visit: &mut impl FnMut(&str, &str, &AccountNode) -> Result<()>,
) -> Result<()> {
    for spec in specs {
        let name = spec
            .name
            .as_deref()
            .ok_or_else(|| Error::InvalidSpec("IDL account is missing its name".into()))?;
        let full_name = if prefix.is_empty() {
            name.to_string()
        } else {
            format!("{prefix}.{name}")
        };
        match &spec.children {
            Some(children) => collect_account_specs(children, &full_name, visit)?,
            None => visit(&full_name, name, spec)?,
        }
    }
    Ok(())
}

fn instruction_discriminator(instruction: &IxDef, method: &str) -> Result<Vec<u8>> {
    if let DiscField::Bytes(entries) = &instruction.discriminator {
        let bytes = entries
            .iter()
            .map(|entry| {
                entry
                    .and_then(|value| u8::try_from(value).ok())
                    .ok_or_else(|| {
                        Error::InvalidSpec(format!("method {method} has an invalid discriminator"))
                    })
            })
            .collect::<Result<Vec<_>>>()?;
        if bytes.len() != 8 {
            return Err(Error::InvalidSpec(format!(
                "method {method} discriminator must contain 8 bytes"
            )));
        }
        return Ok(bytes);
    }

    // Anchor IDLs before v0.30 did not include instruction discriminators.
    // Anchor derives them from the first eight SHA-256 bytes of
    // `global:<rust_instruction_name>`, recovering the snake_case Rust name from
    // the IDL's camelCase. This inverse is exact for ordinary names but can
    // differ when the original had an underscore next to a digit (`claim_2` vs
    // `claim2`); such a name needs a v0.30+ IDL that carries the discriminator.
    let rust_name = crate::utils::camel_to_snake(method);
    Ok(Sha256::digest(format!("global:{rust_name}").as_bytes())[..8].to_vec())
}

#[cfg(test)]
mod tests {
    use {super::*, serde_json::json, solana_keypair::Keypair};

    fn test_idl(discriminator: Option<Value>) -> Value {
        let mut instruction = json!({
            "name": "setConfig",
            "accounts": [
                { "name": "authority", "writable": true, "signer": true },
                {
                    "name": "stateGroup",
                    "accounts": [
                        { "name": "state", "writable": true, "signer": false }
                    ]
                },
                {
                    "name": "systemProgram",
                    "address": "11111111111111111111111111111111",
                    "writable": false,
                    "signer": false
                }
            ],
            "args": [
                { "name": "amount", "type": "u64" },
                { "name": "label", "type": "string" }
            ]
        });
        if let Some(discriminator) = discriminator {
            instruction["discriminator"] = discriminator;
        }
        json!({ "instructions": [instruction] })
    }

    #[test]
    fn builds_instruction_in_idl_order() {
        let scope = Scope::new("http://127.0.0.1:8899");
        let program = Keypair::new().pubkey();
        let payer = Keypair::new();
        let state = Keypair::new().pubkey();
        let instruction = scope
            .program_with_idl(program, test_idl(Some(json!([1, 2, 3, 4, 5, 6, 7, 8]))))
            .method("setConfig")
            .unwrap()
            .payer(&payer)
            .account("authority", payer.pubkey())
            .account("stateGroup.state", state)
            .arg("amount", 42_u64)
            .arg("label", "hello")
            .instruction()
            .unwrap();

        assert_eq!(instruction.program_id, program);
        assert_eq!(instruction.accounts.len(), 3);
        assert_eq!(
            instruction.accounts[0],
            AccountMeta::new(payer.pubkey(), true)
        );
        assert_eq!(instruction.accounts[1], AccountMeta::new(state, false));
        assert_eq!(
            instruction.accounts[2],
            AccountMeta::new_readonly(Address::default(), false)
        );
        assert_eq!(&instruction.data[..8], &[1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(&instruction.data[8..16], &42_u64.to_le_bytes());
        assert_eq!(&instruction.data[16..20], &5_u32.to_le_bytes());
        assert_eq!(&instruction.data[20..], b"hello");
    }

    #[test]
    fn derives_legacy_anchor_discriminator_from_snake_case_name() {
        let model = IdlModel::parse(&test_idl(None));
        let bytes = instruction_discriminator(model.instruction("setConfig").unwrap(), "setConfig")
            .unwrap();
        assert_eq!(bytes, Sha256::digest(b"global:set_config")[..8]);
    }

    #[test]
    fn reports_missing_signer_before_rpc() {
        let scope = Scope::new("http://127.0.0.1:8899");
        let payer = Keypair::new();
        let authority = Keypair::new();
        let state = Keypair::new();
        let error = scope
            .program_with_idl(Keypair::new().pubkey(), test_idl(None))
            .method("setConfig")
            .unwrap()
            .payer(&payer)
            .account("authority", authority.pubkey())
            .account("state", state.pubkey())
            .arg("amount", 1_u64)
            .arg("label", "test")
            .instruction()
            .unwrap_err();

        assert!(matches!(error, Error::MissingSigner { .. }));
    }

    #[test]
    fn account_signer_registers_address_and_signature() {
        let scope = Scope::new("http://127.0.0.1:8899");
        let authority = Keypair::new();
        let state = Keypair::new();
        let instruction = scope
            .program_with_idl(Keypair::new().pubkey(), test_idl(None))
            .method("setConfig")
            .unwrap()
            .payer(&authority)
            .account_signer("authority", &authority)
            .account("state", state.pubkey())
            .args(json!({ "amount": 1, "label": "test" }))
            .unwrap()
            .instruction()
            .unwrap();

        assert!(instruction.accounts[0].is_signer);
    }
}

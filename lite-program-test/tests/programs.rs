use lite_program_test::ProgramTest;

#[test]
pub fn counter() {
    // counter::entrypoint
    // counter::process_instruction(program_id, accounts, instruction_data)
    // ProgramTest::new().deploy_builtin(|vm, _, _, _, _, _| {
    //     let vm = unsafe {
    //         &mut *((vm as *mut u64).offset(-(get_runtime_environment_key() as isize)) as *mut vm::EbpfVm<Invok>)
    //     };
    //     // vm.declare_process_instruction
    //     // counter::process_instruction(program_id, accounts, instruction_data)
    // });
}

/*

pub fn vm $(<$($generic_ident : $generic_type),+>)? (
    $vm: *mut $crate::vm::EbpfVm<$ContextObject>,
    $arg_a: u64,
    $arg_b: u64,
    $arg_c: u64,
    $arg_d: u64,
    $arg_e: u64,
) {
    use $crate::vm::ContextObject;
    let vm = unsafe {
        &mut *(($vm as *mut u64).offset(-($crate::vm::get_runtime_environment_key() as isize)) as *mut $crate::vm::EbpfVm<$ContextObject>)
    };
    let config = vm.loader.get_config();
    if config.enable_instruction_meter {
        vm.context_object_pointer.consume(vm.previous_instruction_meter - vm.due_insn_count);
    }
    let converted_result: $crate::error::ProgramResult = Self::rust $(::<$($generic_ident),+>)?(
        vm.context_object_pointer, $arg_a, $arg_b, $arg_c, $arg_d, $arg_e, &mut vm.memory_mapping,
    ).map_err(|err| $crate::error::EbpfError::SyscallError(err)).into();
    vm.program_result = converted_result;
    if config.enable_instruction_meter {
        vm.previous_instruction_meter = vm.context_object_pointer.get_remaining();
    }
}
*/

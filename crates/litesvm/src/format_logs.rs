use {ansi_term::Colour, std::fmt::Write};

const PROGRAM_LOG: &str = "Program log:";

#[derive(Debug)]
enum Importance {
    Low,
    High,
    VeryHigh,
    Error,
}

fn get_importance(program_source: &str, program_log: &str) -> Importance {
    let log = program_log.to_lowercase();
    if log.contains("error: ")
        || log.contains("error ")
        || log.contains("err: ")
        || log.contains("err ")
        || log.contains("failure: ")
        || log.contains("failure ")
        || log.contains("failed: ")
        || log.contains("failed ")
        || log.contains("fail: ")
        || log.contains("fail ")
    {
        Importance::Error
    } else if log.contains("signer privilege escalated") {
        Importance::High
    } else if program_source == PROGRAM_LOG {
        Importance::VeryHigh
    } else {
        Importance::Low
    }
}

fn colourise(importance: Importance, log: &str) -> String {
    match importance {
        Importance::Error => Colour::Fixed(9).bold().paint(log),
        Importance::VeryHigh => Colour::Green.paint(log),
        Importance::High => Colour::Fixed(243).bold().paint(log),
        Importance::Low => Colour::Fixed(239).paint(log),
    }
    .to_string()
}

fn format_line(line: &str) -> String {
    const PROGRAM: &str = "Program";
    const PROCESS_INSTRUCTION: &str = "process_instruction:";
    const SOLANA_RUNTIME: &str = "solana_runtime:";
    // Check for optional prefixes
    let (program_source, program_log) = match line {
        s if s.starts_with(PROGRAM_LOG) => (PROGRAM_LOG, s[PROGRAM_LOG.len()..].trim_start()),
        s if s.starts_with(PROGRAM) => (PROGRAM, s[PROGRAM.len()..].trim_start()),
        s if s.starts_with(PROCESS_INSTRUCTION) => (
            PROCESS_INSTRUCTION,
            s[PROCESS_INSTRUCTION.len()..].trim_start(),
        ),
        s if s.starts_with(SOLANA_RUNTIME) => {
            (SOLANA_RUNTIME, s[SOLANA_RUNTIME.len()..].trim_start())
        }
        s => ("", s),
    };
    let importance = get_importance(program_source, program_log);
    let log = if ["", PROGRAM_LOG].contains(&program_source) {
        program_log.to_string()
    } else {
        format!("{program_source} {program_log}")
    };
    colourise(importance, &log)
}

pub(crate) fn format_logs(logs: &[String]) -> String {
    let mut out: String = String::new();
    for line in logs {
        if !line.is_empty() {
            let formatted = format_line(line);
            writeln!(&mut out, "{formatted}").unwrap();
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // Examples:
    //
    // ["Program 11111111111111111111111111111111 invoke [1]", "Program 11111111111111111111111111111111 failed: Computational budget exceeded"]
    // ["Program 11111111111111111111111111111111 invoke [1]", "Program 11111111111111111111111111111111 success"]
    // ["Program 11111111111111111111111111111111 invoke [1]", "Program 11111111111111111111111111111111 success", "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA invoke [1]", "Program log: Instruction: InitializeMint2", "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA consumed 2779 of 202850 compute units", "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA success"]
    // ["Program Logging111111111111111111111111111111111111 invoke [1]", "Program log: static string"]
    // ["Program Config1111111111111111111111111111111111111 invoke [1]", "account J2kSTGu6eod7MUAy2nNZhFW5ye5ZdhAri6bcJJHRhhXy signer_key().is_none()", "Program Config1111111111111111111111111111111111111 failed: missing required signature for instruction"]
    // ["Program 1111111QLbz7JHiBTspS962RLKV8GndWFwiEaqKM invoke [1]", "Program log: panicked at clock-example/src/lib.rs:17:5:\nassertion failed: got_clock.unix_timestamp < 100", "Program 1111111QLbz7JHiBTspS962RLKV8GndWFwiEaqKM consumed 1751 of 200000 compute units", "Program 1111111QLbz7JHiBTspS962RLKV8GndWFwiEaqKM failed: SBF program panicked"]
    #[test]
    fn test_format_line() {
        let line = "Program 11111111111111111111111111111111 failed: Computational budget exceeded";
        let formatted = format_line(line);
        assert_eq!(
            formatted,
            "\u{1b}[1;38;5;9mProgram 11111111111111111111111111111111 failed: Computational budget exceeded\u{1b}[0m"
        );
        let line = "Program log: static string";
        let formatted = format_line(line);
        eprintln!("{formatted}");
        assert_eq!(formatted, "\u{1b}[32mstatic string\u{1b}[0m");
    }

    #[test]
    fn test_format_logs() {
        let logs = ["Program 1111111QLbz7JHiBTspS962RLKV8GndWFwiEaqKM invoke [1]", "Program log: panicked at clock-example/src/lib.rs:17:5:\nassertion failed: got_clock.unix_timestamp < 100", "Program 1111111QLbz7JHiBTspS962RLKV8GndWFwiEaqKM consumed 1751 of 200000 compute units", "Program 1111111QLbz7JHiBTspS962RLKV8GndWFwiEaqKM failed: SBF program panicked"].map(ToString::to_string);
        let formatted = format_logs(&logs);
        assert_eq!(
            formatted,
            "\u{1b}[38;5;239mProgram 1111111QLbz7JHiBTspS962RLKV8GndWFwiEaqKM invoke [1]\u{1b}[0m\n\u{1b}[1;38;5;9mpanicked at clock-example/src/lib.rs:17:5:\nassertion failed: got_clock.unix_timestamp < 100\u{1b}[0m\n\u{1b}[38;5;239mProgram 1111111QLbz7JHiBTspS962RLKV8GndWFwiEaqKM consumed 1751 of 200000 compute units\u{1b}[0m\n\u{1b}[1;38;5;9mProgram 1111111QLbz7JHiBTspS962RLKV8GndWFwiEaqKM failed: SBF program panicked\u{1b}[0m\n"
        );
    }
}

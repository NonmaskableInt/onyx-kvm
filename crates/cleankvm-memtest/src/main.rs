use std::env;
use std::process::ExitCode;
use std::time::Instant;

const DEFAULT_MIB: usize = 64;
const DEFAULT_ITERATIONS: usize = 8;
const ADDRESS_SALT: u64 = 0xd6e8_feb8_6659_fd93;

#[derive(Clone, Copy)]
struct Config {
    mib: usize,
    iterations: usize,
}

fn main() -> ExitCode {
    match parse_config(env::args().skip(1)) {
        Ok(config) => match run(config) {
            Ok(()) => ExitCode::SUCCESS,
            Err(message) => {
                eprintln!("FAIL: {message}");
                ExitCode::FAILURE
            }
        },
        Err(message) => {
            eprintln!("{message}");
            eprintln!("usage: cleankvm-memtest [--mib N] [--iterations N]");
            ExitCode::from(2)
        }
    }
}

fn parse_config(args: impl Iterator<Item = String>) -> Result<Config, String> {
    let mut config = Config {
        mib: DEFAULT_MIB,
        iterations: DEFAULT_ITERATIONS,
    };
    let mut args = args;
    while let Some(argument) = args.next() {
        let value = args
            .next()
            .ok_or_else(|| format!("missing value for {argument}"))?;
        let parsed = value
            .parse::<usize>()
            .map_err(|_| format!("invalid integer for {argument}: {value}"))?;
        if parsed == 0 {
            return Err(format!("{argument} must be greater than zero"));
        }
        match argument.as_str() {
            "--mib" => config.mib = parsed,
            "--iterations" => config.iterations = parsed,
            _ => return Err(format!("unknown argument: {argument}")),
        }
    }
    Ok(config)
}

fn run(config: Config) -> Result<(), String> {
    let bytes = config
        .mib
        .checked_mul(1024 * 1024)
        .ok_or_else(|| "requested allocation is too large".to_owned())?;
    let words = bytes / size_of::<u64>();
    let mut memory = Vec::new();
    memory
        .try_reserve_exact(words)
        .map_err(|error| format!("cannot reserve {bytes} bytes: {error}"))?;
    memory.resize(words, 0);

    println!(
        "CleanKVM memory qualification: {} MiB, {} iterations, {} words",
        config.mib, config.iterations, words
    );
    let started = Instant::now();
    for iteration in 1..=config.iterations {
        address_test(&mut memory)?;
        march_c_minus(&mut memory)?;
        println!("iteration {iteration}/{}: PASS", config.iterations);
    }
    println!("PASS: completed in {} ms", started.elapsed().as_millis());
    Ok(())
}

fn address_pattern(index: usize) -> u64 {
    (index as u64).wrapping_mul(ADDRESS_SALT).rotate_left(23) ^ ADDRESS_SALT
}

fn address_test(memory: &mut [u64]) -> Result<(), String> {
    for (index, word) in memory.iter_mut().enumerate() {
        *word = address_pattern(index);
    }
    verify(memory, address_pattern, "address")?;

    for (index, word) in memory.iter_mut().enumerate() {
        *word = !address_pattern(index);
    }
    verify(memory, |index| !address_pattern(index), "inverse-address")
}

fn march_c_minus(memory: &mut [u64]) -> Result<(), String> {
    memory.fill(0);
    read_then_write(
        memory.iter_mut().enumerate(),
        0,
        u64::MAX,
        "march-up-0-to-1",
    )?;
    read_then_write(
        memory.iter_mut().enumerate(),
        u64::MAX,
        0,
        "march-up-1-to-0",
    )?;
    read_then_write(
        memory.iter_mut().enumerate().rev(),
        0,
        u64::MAX,
        "march-down-0-to-1",
    )?;
    read_then_write(
        memory.iter_mut().enumerate().rev(),
        u64::MAX,
        0,
        "march-down-1-to-0",
    )?;
    verify(memory, |_| 0, "march-final-zero")
}

fn read_then_write<'a>(
    words: impl Iterator<Item = (usize, &'a mut u64)>,
    expected: u64,
    replacement: u64,
    phase: &str,
) -> Result<(), String> {
    for (index, word) in words {
        if *word != expected {
            return Err(mismatch(phase, index, expected, *word));
        }
        *word = replacement;
    }
    Ok(())
}

fn verify(memory: &[u64], expected: impl Fn(usize) -> u64, phase: &str) -> Result<(), String> {
    for (index, actual) in memory.iter().copied().enumerate() {
        let expected = expected(index);
        if actual != expected {
            return Err(mismatch(phase, index, expected, actual));
        }
    }
    Ok(())
}

fn mismatch(phase: &str, index: usize, expected: u64, actual: u64) -> String {
    format!(
        "{phase} mismatch at word {index} (byte offset 0x{:x}): expected 0x{expected:016x}, got 0x{actual:016x}",
        index * size_of::<u64>()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn address_and_march_tests_pass_on_small_buffer() {
        let mut memory = vec![0_u64; 257];
        assert!(address_test(&mut memory).is_ok());
        assert!(march_c_minus(&mut memory).is_ok());
    }

    #[test]
    fn verification_reports_the_exact_offset() {
        let memory = [0_u64, 1];
        let error = match verify(&memory, |_| 0, "test") {
            Ok(()) => panic!("corruption must fail"),
            Err(error) => error,
        };
        assert!(error.contains("word 1"));
        assert!(error.contains("byte offset 0x8"));
    }

    #[test]
    fn rejects_zero_sized_configuration() {
        let args = ["--mib".to_owned(), "0".to_owned()].into_iter();
        assert!(parse_config(args).is_err());
    }
}

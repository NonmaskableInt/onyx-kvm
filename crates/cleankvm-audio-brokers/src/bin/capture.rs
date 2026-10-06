use std::process::ExitCode;

fn main() -> ExitCode {
    cleankvm_audio_brokers::run(cleankvm_audio_brokers::Kind::Capture)
}

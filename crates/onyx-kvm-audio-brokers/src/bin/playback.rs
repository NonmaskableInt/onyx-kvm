use std::process::ExitCode;

fn main() -> ExitCode {
    onyx_kvm_audio_brokers::run(onyx_kvm_audio_brokers::Kind::Playback)
}

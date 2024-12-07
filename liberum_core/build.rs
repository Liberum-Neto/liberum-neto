
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::compile_protos("src/test_runner/api/protos/test_protocol.proto")?;
    Ok(())
}
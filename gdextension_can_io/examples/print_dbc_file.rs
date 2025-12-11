use can_dbc::Dbc;
use std::{io, path};

fn main() -> io::Result<()> {
    let path = "./examples/sample.dbc";
    let data = std::fs::read_to_string(path)?;

    let dbc = Dbc::try_from(data.as_str()).expect("Failed to parse dbc file");

    println!("Reading dbc file at: {:?}", path);

    for message in &dbc.messages {
        println!("====");
        println!("Name: {:?}", message.name);
        println!("Id: {:?}", message.id);
        println!("Size: {:?}", message.size);
        println!("Transmitter: {:?}", message.transmitter);
        for signal in &message.signals {
            let ext_type = dbc.extended_value_type_for_signal(message.id, &signal.name);

            println!("  {:?}", signal);
            println!("      Type: {:?}", ext_type);
        }
    }

    Ok(())
}

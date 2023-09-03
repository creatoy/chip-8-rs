use chip_8;
use std::env;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::thread::sleep;
use std::time::{Duration, SystemTime};

fn main() {
    let args: Vec<_> = env::args().collect();
    if args.len() < 2 {
        println!("Usage: {} <path_to_rom>", args[0]);
        return;
    }

    let path = Path::new(&args[1]);
    println!("Loading rom file: {}", path.display());

    let mut file = match File::open(&path) {
        Ok(file) => file,
        Err(e) => panic!("Couldn't open {:?}: {}", path, e),
    };

    let mut bin = Vec::new();
    file.read_to_end(&mut bin).unwrap();

    let mut cpu = chip::Chip::new(
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    );

    cpu.load_rom(chip::ENTRY_ADDR, &bin).unwrap();

    let mut display = frontend::Display::new(16);

    loop {
        match display.update(&mut cpu) {
            Err(chip::Exception::Halt(0)) => break,
            Err(e) => {
                println!("Error {:?} occured!", e);
                break;
            }
            Ok(_) => (),
        }

        // println!("======== CHIP-8 Debug Info =========");
        // println!("{}", cpu);
        // println!("====================================");
        sleep(Duration::from_millis(10));
    }
}

use std::process::ExitCode;

use crate::{
    args::{ParseArgs, ValidateArgs},
    parser::parse_pcad,
};

pub fn run_parse(args: ParseArgs) -> ExitCode {
    let source = match crate::read_source(args.input.as_deref()) {
        Ok(source) => source,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::FAILURE;
        }
    };

    match parse_pcad(&source) {
        Ok(ast) => {
            if args.json {
                match crate::codegen::compile_to_three_json(&ast) {
                    Ok(json) => {
                        println!("{json}");
                    }
                    Err(err) => {
                        eprintln!("{err}");
                        return ExitCode::FAILURE;
                    }
                }
            } else if args.ast {
                println!("{ast:#?}");
            } else {
                println!("Parsed {} declaration(s)", ast.decls.len());
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
    }
}

pub fn run_validate(args: ValidateArgs) -> ExitCode {
	let source = match crate::read_source(args.input.as_deref()) {
		Ok(source) => source,
		Err(err) => {
			eprintln!("{err}");
			return ExitCode::FAILURE;
		}
	};

	match parse_pcad(&source) {
		Ok(ast) => {
			let mut evaluator = crate::eval::Evaluator::new(&ast);
			match evaluator.build() {
				Ok(_) => {
					println!("pcad file is valid");
					ExitCode::SUCCESS
				}
				Err(err) => {
					eprintln!("{err}");
					ExitCode::FAILURE
				}
			}
		}
		Err(err) => {
			eprintln!("{err}");
			ExitCode::FAILURE
		}
	}
}

#[cfg(test)]
mod tests {
	use std::{fs, path::PathBuf, time::{SystemTime, UNIX_EPOCH}};

	use super::*;

	#[test]
	fn validates_a_valid_file() {
		let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
		let input = manifest_dir.join("../examples/puppybot.pcad");
		let exit = run_validate(ValidateArgs { input: Some(input) });
		assert_eq!(exit, ExitCode::SUCCESS);
	}

	#[test]
	fn validates_reports_semantic_errors() {
		let invalid = std::env::temp_dir().join(format!(
			"puppycad-invalid-{}.pcad",
			SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
		));
		let bad_source = r#"
solid body = box {
  w: 1;
  h: 2;
  d: 3;
}

feature bad = hole {
  target: body.unknown;
  x: 1;
  y: 1;
  d: 1;
}
"#;

		fs::write(&invalid, bad_source).unwrap();
		let exit = run_validate(ValidateArgs {
			input: Some(invalid.clone()),
		});
		let _ = fs::remove_file(&invalid);
		assert_eq!(exit, ExitCode::FAILURE);
	}
}

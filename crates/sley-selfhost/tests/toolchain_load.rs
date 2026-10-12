//! The loader decodes all of S into the inventory.
mod common;
use common::*;

#[test]
fn loads_the_toolchain_inventory() {
    let s = toolchain_s("cd_object");
    let (value, instructions) = run_counted(
        "t_load_counts",
        vec![objects_input(&s), bytes(s.epoch.as_bytes())],
    );
    let counts = tuple_items(&result(value).expect("loads"));
    let (program, _) = program();
    assert_eq!(as_u64(&counts[0]), program.functions.len() as u64);
    assert_eq!(as_u64(&counts[1]), program.operations.len() as u64);
    assert_eq!(as_u64(&counts[2]), s.objects.len() as u64);
    eprintln!("loaded {} objects in {instructions} instructions", s.objects.len());
}

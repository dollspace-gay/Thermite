extern crate thermite_probe;

fn main() {
    assert_eq!(thermite_probe::probe_shell::boot_observation(), 1);
}

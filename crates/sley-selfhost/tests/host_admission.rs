//! Pinned-identity host admission (`HOST_ABI_V3`): the SH2 host admits a v3
//! package only when its package identity is pinned.
use sley_selfhost::canonical;
use sley_selfhost::native;
use sley_selfhost::seed::{Module, add, t, u64c, v};
use sley_vm::host_admission::{HostAdmissionError, admit_pinned_v3_package};

fn small_package() -> native::C0Build {
    let mut m = Module::new("host-admission");
    m.func("main", &[("n", t::u64())], t::u64(), |f| {
        f.ret(add(v("n"), u64c(1)));
    });
    let (program, names) = m.finish();
    let s = canonical::build(&program, names["main"], "host-admission");
    native::c0_build(&s, native::toolchain_limits()).expect("C0 builds")
}

#[test]
fn unpinned_package_is_refused() {
    let built = small_package();
    let refused = admit_pinned_v3_package(&built.package, &[]).unwrap_err();
    assert_eq!(refused, HostAdmissionError::NotPinned);
    assert_eq!(refused.to_string(), "HOST_ADMISSION_NOT_PINNED");
    let other = [[7_u8; 32]];
    assert_eq!(
        admit_pinned_v3_package(&built.package, &other)
            .unwrap_err()
            .to_string(),
        "HOST_ADMISSION_NOT_PINNED"
    );
}

#[test]
fn pinned_package_is_admitted() {
    let built = small_package();
    let pinned = [built.digests.package_digest];
    let (digests, _approved) = admit_pinned_v3_package(&built.package, &pinned).expect("admitted");
    assert_eq!(digests.package_digest, built.digests.package_digest);
}

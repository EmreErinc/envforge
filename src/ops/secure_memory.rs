use zeroize::Zeroize;

#[cfg(unix)]
pub fn disable_core_dumps() {
    let rlim = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    let _ = unsafe { libc::setrlimit(libc::RLIMIT_CORE, &rlim) };
}

#[cfg(not(unix))]
pub fn disable_core_dumps() {}

pub fn zeroize_strings(strings: &mut Vec<String>) {
    for s in strings.iter_mut() {
        s.zeroize();
    }
    strings.clear();
}

pub fn zeroize_vec_u8(data: &mut Vec<u8>) {
    data.zeroize();
    data.clear();
}

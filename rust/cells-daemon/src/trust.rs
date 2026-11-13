//! Who may talk to the engine. The kernel says which process connected: its user
//! (`getpeereid` on a Mac, `SO_PEERCRED` on Linux), and on a Mac its audit token, which
//! names the exact process (a pid could be reused before the check runs). That process's
//! code signature is then held to the engine's own:
//!
//! - A signed engine (Cells.app's `CellsEngine.app`, or the `cells` command inside the app
//!   serving) admits a client signed by its own team as the app or its command: `<app id>`
//!   or `<app id>.cli`, the app id read from the engine's own identifier (`<app id>.engine`
//!   or `<app id>.cli`). Any other program of the same user, a script or a binary dropped in
//!   a folder, is refused before it says a word: the engine is what writes a person's books.
//! - An engine with no team (`cargo build`, a development build run by hand) has no
//!   signature to hold anyone to: it admits a process of the same user, and says so when it
//!   starts.

use std::os::unix::net::UnixStream;

pub enum Trust {
    SameUser,
    /// The code-signing requirement a client must meet.
    Signed(String),
}

impl Trust {
    /// What this process's own signature asks of a client.
    pub fn own() -> Trust {
        match signing::own() {
            Some((team, identifier)) => {
                let app = identifier.strip_suffix(".engine").or_else(|| identifier.strip_suffix(".cli")).unwrap_or(&identifier);
                Trust::Signed(format!(
                    r#"anchor apple generic and certificate leaf[subject.OU] = "{team}" and (identifier "{app}" or identifier "{app}.cli")"#
                ))
            }
            None => Trust::SameUser,
        }
    }

    /// The rule, as the trace says it at start.
    pub fn said(&self) -> String {
        match self {
            Trust::SameUser => "a process of this user (the engine is unsigned)".into(),
            Trust::Signed(requirement) => format!("a process of this user meeting `{requirement}`"),
        }
    }

    /// Whether the process at the other end of `stream` may talk to the engine; why not.
    pub fn admits(&self, stream: &UnixStream) -> Result<(), String> {
        let peer = same_user(stream)?;
        match self {
            Trust::SameUser => Ok(()),
            Trust::Signed(requirement) => signing::peer_meets(stream, requirement).map_err(|why| format!("pid {peer}: {why}")),
        }
    }
}

/// The peer's pid when it runs as this user: the kernel's word.
#[cfg(target_os = "macos")]
fn same_user(stream: &UnixStream) -> Result<i32, String> {
    use std::os::fd::AsRawFd;
    let (mut uid, mut gid) = (0, 0);
    // SAFETY: getpeereid writes two integers through the pointers it is handed, both ours.
    if unsafe { libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) } != 0 {
        return Err(format!("its user could not be read: {}", std::io::Error::last_os_error()));
    }
    // SAFETY: getuid has no failure and touches nothing.
    if uid != unsafe { libc::getuid() } {
        return Err(format!("another user (uid {uid})"));
    }
    let mut pid: libc::pid_t = 0;
    let mut len = std::mem::size_of_val(&pid) as libc::socklen_t;
    // SAFETY: LOCAL_PEERPID writes one pid_t into the buffer it is handed, ours.
    let got = unsafe {
        libc::getsockopt(stream.as_raw_fd(), 0 /* SOL_LOCAL */, libc::LOCAL_PEERPID, (&mut pid as *mut libc::pid_t).cast(), &mut len)
    };
    Ok(if got == 0 { pid } else { -1 })
}

/// The peer's pid when it runs as this user: the kernel's word, all three ids in one read.
#[cfg(target_os = "linux")]
fn same_user(stream: &UnixStream) -> Result<i32, String> {
    use std::os::fd::AsRawFd;
    let mut cred = libc::ucred { pid: 0, uid: 0, gid: 0 };
    let mut len = std::mem::size_of_val(&cred) as libc::socklen_t;
    // SAFETY: SO_PEERCRED writes one ucred into the buffer it is handed, ours.
    let got = unsafe {
        libc::getsockopt(stream.as_raw_fd(), libc::SOL_SOCKET, libc::SO_PEERCRED, (&mut cred as *mut libc::ucred).cast(), &mut len)
    };
    if got != 0 {
        return Err(format!("its user could not be read: {}", std::io::Error::last_os_error()));
    }
    // SAFETY: getuid has no failure and touches nothing.
    if cred.uid != unsafe { libc::getuid() } {
        return Err(format!("another user (uid {})", cred.uid));
    }
    Ok(cred.pid)
}

#[cfg(target_os = "macos")]
mod signing {
    use std::os::fd::AsRawFd;
    use std::os::unix::net::UnixStream;

    use core_foundation::base::{CFType, TCFType};
    use core_foundation::data::CFData;
    use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
    use core_foundation::string::{CFString, CFStringRef};
    use security_framework_sys::code_signing::*;

    // What the crate does not bind: a code object's own signing information.
    unsafe extern "C" {
        fn SecCodeCopySigningInformation(code: SecCodeRef, flags: u32, information: *mut CFDictionaryRef) -> i32;
        static kSecCodeInfoTeamIdentifier: CFStringRef;
        static kSecCodeInfoIdentifier: CFStringRef;
    }
    /// `kSecCSSigningInformation`: the team and identifier included.
    const SIGNING_INFORMATION: u32 = 1 << 1;

    /// This process's team and signing identifier: nothing when it has no team (unsigned,
    /// or signed ad hoc).
    pub fn own() -> Option<(String, String)> {
        let mut code: SecCodeRef = std::ptr::null_mut();
        // SAFETY: SecCodeCopySelf writes one owned reference into `code`, released by the wrap.
        if unsafe { SecCodeCopySelf(0, &mut code) } != 0 {
            return None;
        }
        let code = unsafe { CFType::wrap_under_create_rule(code as _) };
        let mut info: CFDictionaryRef = std::ptr::null();
        // SAFETY: as above, one owned dictionary written into `info`.
        let status = unsafe { SecCodeCopySigningInformation(code.as_CFTypeRef() as SecCodeRef, SIGNING_INFORMATION, &mut info) };
        assert_eq!(status, 0, "this process's own signing information is always readable");
        let info: CFDictionary<CFString, CFType> = unsafe { CFDictionary::wrap_under_create_rule(info) };
        let text = |key: CFStringRef| {
            let key = unsafe { CFString::wrap_under_get_rule(key) };
            info.find(&key).and_then(|v| v.downcast::<CFString>()).map(|s| s.to_string())
        };
        Some((text(unsafe { kSecCodeInfoTeamIdentifier })?, text(unsafe { kSecCodeInfoIdentifier })?))
    }

    /// Whether the process at the other end of `stream` meets `requirement`, its bundle on
    /// the disk exactly what was signed.
    pub fn peer_meets(stream: &UnixStream, requirement: &str) -> Result<(), String> {
        let mut token = [0u32; 8];
        let mut len = std::mem::size_of_val(&token) as libc::socklen_t;
        // SAFETY: LOCAL_PEERTOKEN writes the 32-byte audit token into the buffer, ours.
        let got = unsafe {
            libc::getsockopt(stream.as_raw_fd(), 0 /* SOL_LOCAL */, libc::LOCAL_PEERTOKEN, token.as_mut_ptr().cast(), &mut len)
        };
        if got != 0 {
            return Err(format!("its audit token could not be read: {}", std::io::Error::last_os_error()));
        }
        assert_eq!(len as usize, std::mem::size_of_val(&token), "an audit token is 32 bytes");
        let bytes: Vec<u8> = token.iter().flat_map(|w| w.to_ne_bytes()).collect();
        let attributes = CFDictionary::from_CFType_pairs(&[(
            unsafe { CFString::wrap_under_get_rule(kSecGuestAttributeAudit) },
            CFData::from_buffer(&bytes).into_CFType(),
        )]);
        let mut code: SecCodeRef = std::ptr::null_mut();
        // SAFETY: one owned code reference written into `code`, released by the wrap.
        let status = unsafe { SecCodeCopyGuestWithAttributes(std::ptr::null_mut(), attributes.as_concrete_TypeRef(), 0, &mut code) };
        if status != 0 {
            return Err(format!("its code could not be found (OSStatus {status})"));
        }
        let code = unsafe { CFType::wrap_under_create_rule(code as _) };
        let mut req: SecRequirementRef = std::ptr::null_mut();
        // SAFETY: one owned requirement written into `req`, released by the wrap.
        let status = unsafe { SecRequirementCreateWithString(CFString::new(requirement).as_concrete_TypeRef(), 0, &mut req) };
        assert_eq!(status, 0, "the requirement {requirement:?} parses");
        let req = unsafe { CFType::wrap_under_create_rule(req as _) };
        let status = unsafe {
            SecCodeCheckValidity(code.as_CFTypeRef() as SecCodeRef, kSecCSStrictValidate, req.as_CFTypeRef() as SecRequirementRef)
        };
        if status != 0 {
            return Err(format!("not signed as Cells is (OSStatus {status})"));
        }
        Ok(())
    }
}

#[cfg(not(target_os = "macos"))]
mod signing {
    use std::os::unix::net::UnixStream;

    /// No code signatures outside a Mac: an engine there admits by user alone.
    pub fn own() -> Option<(String, String)> {
        None
    }

    pub fn peer_meets(_: &UnixStream, requirement: &str) -> Result<(), String> {
        unreachable!("no requirement is made without a signature: {requirement}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_test_binary_is_unsigned_and_admits_its_own_user() {
        let trust = Trust::own();
        assert!(matches!(trust, Trust::SameUser), "a cargo test binary has no team");
        let (a, b) = UnixStream::pair().unwrap();
        trust.admits(&a).unwrap();
        // a requirement no code meets refuses this process, read from the real peer
        #[cfg(target_os = "macos")]
        assert!(Trust::Signed("never".into()).admits(&b).is_err());
    }
}

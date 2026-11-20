use std::ffi::CStr;
use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InterfaceAddress {
    pub name: String,
    pub address: IpAddr,
}

pub fn interface_addresses() -> io::Result<Vec<InterfaceAddress>> {
    let mut list: *mut libc::ifaddrs = std::ptr::null_mut();
    if unsafe { libc::getifaddrs(&raw mut list) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let mut found = Vec::new();
    let mut entry = list;
    while !entry.is_null() {
        let current = unsafe { &*entry };
        entry = current.ifa_next;
        if current.ifa_name.is_null() || current.ifa_addr.is_null() {
            continue;
        }
        let Ok(name) = unsafe { CStr::from_ptr(current.ifa_name) }.to_str() else {
            continue;
        };
        if let Some(address) = unsafe { read_address(current.ifa_addr) } {
            found.push(InterfaceAddress {
                name: name.to_owned(),
                address,
            });
        }
    }
    unsafe { libc::freeifaddrs(list) };
    Ok(found)
}

unsafe fn read_address(address: *const libc::sockaddr) -> Option<IpAddr> {
    match i32::from(unsafe { (*address).sa_family }) {
        libc::AF_INET => {
            let address = unsafe { &*address.cast::<libc::sockaddr_in>() };
            Some(IpAddr::V4(Ipv4Addr::from(u32::from_be(
                address.sin_addr.s_addr,
            ))))
        }
        libc::AF_INET6 => {
            let address = unsafe { &*address.cast::<libc::sockaddr_in6>() };
            Some(IpAddr::V6(Ipv6Addr::from(address.sin6_addr.s6_addr)))
        }
        _ => None,
    }
}

use std::net::IpAddr;

fn main() {
    let local_ip = local_ip_address::local_ip().ok();
    println!("local_ip: {:?}", local_ip);
    
    let all = local_ip_address::list_afinet_netifas().unwrap();
    println!("all IPs: {:?}", all);
}

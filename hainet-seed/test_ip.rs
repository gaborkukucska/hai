fn main() {
    let local_ip = local_ip_address::local_ip().unwrap();
    println!("local_ip(): {}", local_ip);
    let all = local_ip_address::list_afinet_netifas().unwrap();
    for (name, ip) in all {
        println!("{}: {}", name, ip);
    }
}

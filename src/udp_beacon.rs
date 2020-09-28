// Copyright 2019, 2020 Rohde & Schwarz GmbH & Co KG
//      philipp.stanner@rohde-schwarz.com
//      hagen.pfeifer@rohde-schwarz.com
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::net::UdpSocket;
use std::io::Error;

use crate::{TracerContext, SERVER_VERSION, PROTOCOLL_VERSION};
use crate::tcp_handler::MAGIC_NUMB;


// Bind to a interface for udp announcements, if the user specified one
// Bind to default otherwise
pub(crate) fn init(iface: Option<String>) -> Result<UdpSocket, Error>
{
    let sock;
    if let Some(addr) = iface {
        sock = UdpSocket::bind((&addr[..], 0))?; // bind ephemeric port
    } else {
        sock = UdpSocket::bind(("0.0.0.0", 0))?;
    }

    Ok(sock)
}


pub(crate) fn announce_tracer(ctx: &mut TracerContext)
{
    let mut announce_msg: Vec<u8> = Vec::with_capacity(256);

    let tracer_data = format_json(&ctx);

    announce_msg.extend_from_slice(&MAGIC_NUMB);
    announce_msg.extend_from_slice(&tracer_data.as_bytes());

    if let Some(sock) = &ctx.udp_sock {
        let _ = sock.send_to(&announce_msg, ctx.app_cfg.announce_addr.unwrap());
    }

    ctx.sequence_no += 1;
}


fn format_json(ctx: &TracerContext) -> String
{
    let mut announce_interval: u64 = ctx.app_cfg.announce_interval.as_secs();
    announce_interval += ctx.app_cfg.announce_interval.subsec_millis() as u64;
    let s = format!("{{ \"sequence_nr\": {},\
                \"server_version\": \"{}\", \"protocoll_version\": \"{}\",\
                \"update_interval_msecs\": {},\
                \"hostname\": \"{}\", \"process_name\": \"{}\",\
                \"port\": {}}}",
                ctx.sequence_no, SERVER_VERSION, PROTOCOLL_VERSION,
                announce_interval, ctx.app_cfg.hostname,
                ctx.app_cfg.process_name,
                ctx.listener.local_addr().unwrap().port());

    String::from(s)
}

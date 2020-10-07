-- Fancy Wireshark Protocol Dissector for tracy
-- 
-- This Source Code Form is subject to the terms of the Mozilla Public
-- License, v. 2.0. If a copy of the MPL was not distributed with this
-- file, You can obtain one at https://mozilla.org/MPL/2.0/.
--
-- Usage: 
--      Linux  : Copy or symlink this file to "~/.local/lib/wireshark/plugins/tracy.lua".
--      Windows: Copy this file to "%APPDATA%\Wireshark\plugins".
--
-- More information about plugin paths are available here:
-- https://www.wireshark.org/docs/wsug_html_chunked/ChPluginFolders.html
--
-- The TRACY TCP Port can be changed in the protocol settings via the GUI.

tracy_proto = Proto("tracy", "TRACY Process Tracing Protocol")
tracy_proto.prefs.tcp_ports = Pref.string("tcp_port", 0, "TCP Port")
tracy_discover_proto = Proto("tracy_discover", "TRACY Discovery")

local settings = {
    tcp_ports = {}
}

-- Value Strings
local vs_types = {
    [0x01] = "List Request",
    [0x02] = "List Reply",
    [0x03] = "Enable Request",
    [0x04] = "Disable Request",
    [0x05] = "Push",
}

local tracy_info = {
    author = "Stefan Tatschner <stefan@rumpelsepp.org>",
    version = "0.1"
}

set_plugin_info(tracy_info)

local header_len = 12

-- Fields
local f_magic_number = ProtoField.uint32("tracy.magic", "Magic Number", base.HEX)
local f_flags = ProtoField.uint16("tracy.flags", "Flags", base.HEX)
local f_cmd_number = ProtoField.uint16("tracy.cmd", "Command ID", base.HEX, vs_types)
local f_total_len = ProtoField.uint32("tracy.len", "Packet Length", base.DEC)
local f_payload = ProtoField.bytes("tracy.payload", "Payload")
local f_payload_len = ProtoField.uint16("tracy.payload.len", "Payload Length", base.DEC)
local f_name = ProtoField.string("tracy.tracepoint.name", "Tracepoint Name")
local f_name_len = ProtoField.uint16( "tracy.tracepoint.name.len", "Name Length", base.HEX)
local f_list_reply_proto = ProtoField.protocol("tracy.list_reply", "TRACE_LIST_REPLY")
local f_enable_proto = ProtoField.protocol("tracy.enable", "TRACE_ENABLE")
local f_disable_proto = ProtoField.protocol("tracy.disable", "TRACE_DISABLE")
local f_push_proto = ProtoField.protocol("tracy.push", "TRACE_PUSH")
local f_timestamp = ProtoField.uint64("tracy.timestamp", "Timestamp", base.DEC)

tracy_proto.fields = {
    f_magic_number,
    f_flags,
    f_cmd_number,
    f_total_len,
    f_payload,
    f_payload_len,
    f_name,
    f_name_len,
    f_list_reply_proto,
    f_enable_proto,
    f_disable_proto,
    f_push_proto,
    f_timestamp,
    f_push_payload,
}

function _get_length(tvb, pinfo, offset)
    return tvb(offset + 8, 4):uint() + 12
end

function _dissect_tracepoint_list(tvb, pinfo, tree, proto)
    local names = {}
    local offset = header_len
    while offset < tvb:len() do 
        local t = tree:add(proto, tvb())
        local name_len = tvb(offset, 2)
        offset = offset + 2
        local name = tvb(offset, name_len:uint())
        offset = offset + name_len:uint()

        names[#names+1] = name:string()

        t:add(f_name_len, name_len)
        t:add(f_name, name)
    end

    return names
end

function _dissect_push_payload(tvb, pinfo, tree)
    local names = {}
    local offset = header_len
    while offset < tvb:len() do
        local t = tree:add(f_push_proto, tvb(header_len, tvb:len() - header_len))

        local name_len = tvb(offset, 2)
        offset = offset + 2
        local name = tvb(offset, name_len:uint())
        offset = offset + name_len:uint()
        local timestamp = tvb(offset, 8)
        offset = offset + 8
        local data_len = tvb(offset, 2)
        offset = offset + 2
        local payload = tvb(offset, data_len:uint())
        offset = offset + payload:len()

        table.insert(names, name:string())

        t:add(f_name_len, name_len)
        t:add(f_name, name)
        t:add(f_timestamp, timestamp)
        t:add(f_payload_len, data_len)
        t:add(f_payload, payload)
    end

    return names
end

function _dissect(tvb, pinfo, tree)
    pinfo.cols['protocol'] = 'TRACY'
    local t = tree:add(tracy_proto, tvb())
    local magic_number = tvb(0, 4)
    local flags = tvb(4, 2)
    local cmd_number = tvb(6, 2)
    local len = tvb(8, 4)
    local payload = tvb(12, tvb:len()-12)

    t:add(f_magic_number, magic_number)
    t:add(f_flags, flags)
    t:add(f_cmd_number, cmd_number)
    t:add(f_total_len, len)

    local info = ""
    local names = {}
    if cmd_number:uint() == 0x01 then
        info = 'TRACEPOINT_LIST_REQUEST'
    elseif cmd_number:uint() == 0x02 then
        info = 'TRACEPOINT_LIST_REPLY'
        names = _dissect_tracepoint_list(tvb, pinfo, tree, f_list_reply_proto)
    elseif cmd_number:uint() == 0x03 then
        info = "TRACEPOINT_ENABLE_REQUEST"
        names = _dissect_tracepoint_list(tvb, pinfo, tree, f_enable_proto)
    elseif cmd_number:uint() == 0x04 then
        info = "TRACEPOINT_DISABLE_REQUEST"
        names = _dissect_tracepoint_list(tvb, pinfo, tree, f_disable_proto)
    elseif cmd_number:uint() == 0x05 then
        info = "TRACE_PUSH"
        names = _dissect_push_payload(tvb(), pinfo, tree)
    end

    if #names == 1 then
         info = string.format('%s: %s', info, names[1])
    elseif #names == 2 then
         info = string.format('%s: %s, %s', info, names[1], names[2])
    elseif #names > 2 then
         info = string.format('%s: %s, %s, …', info, names[1], names[2])
    end
    pinfo.cols['info'] = info

    return tvb:len()
end

function tracy_proto.dissector(tvb, pinfo, tree)
    dissect_tcp_pdus(tvb, tree, header_len, _get_length, _dissect, true)
end

-- Tracy Discovery Protocol
local f_disco_magic_number = ProtoField.uint32("tracy.magic", "Magic Number", base.HEX)
local f_disco_payload = ProtoField.bytes("tracy.payload", "Payload")
tracy_discover_proto.fields = {
    f_disco_magic_number,
    f_disco_payload,
}

local json_dissector = Dissector.get("json")

function tracy_discover_proto.dissector(tvb, pinfo, tree)
    pinfo.cols.info = "TRACY DISCOVER"
    local magic_number = tvb(0, 4)
    local payload = tvb(4, tvb:len() - 4)
    local t = tree:add(tracy_discover_proto, tvb())

    t:add(f_magic_number, magic_number)
    t:add(f_payload, payload)
    json_dissector:call(payload:tvb(), pinfo, tree)
    return tvb:len()
end

function tracy_proto.prefs_changed()
    tcp_table = DissectorTable.get("tcp.port")
    for i = 1,#settings.tcp_ports,1 do
        tcp_table:remove(settings.tcp_ports[i], tracy_proto)
    end
    settings.tcp_ports = {}
    for port in tracy_proto.prefs.tcp_ports:gmatch('[^,%s]+') do
        p = tonumber(port)
        table.insert(settings.tcp_ports, p)
        tcp_table:add(p, tracy_proto)
    end
end
udp_table = DissectorTable.get("udp.port")
udp_table:add(64042, tracy_discover_proto)

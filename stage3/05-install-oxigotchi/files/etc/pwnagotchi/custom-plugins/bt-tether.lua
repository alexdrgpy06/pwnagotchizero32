-- bt-tether.lua - Bluetooth PAN tethering plugin for pwnagotchi-zero
-- Compatible with pwnagotchi plugin API

local plugin = {
    name = "bt-tether",
    version = "1.0.0",
    author = "pwnagotchi-zero",
    description = "Bluetooth PAN tethering for internet backhaul",
    config = {
        enabled = true,
        auto_reconnect = true,
        show_on_screen = true,
        show_mini_status = true,
        mini_status_position = {110, 0},
        show_detailed_status = true,
        detailed_status_position = {0, 82},
        phone_mac = "",
        reconnect_interval = 30,
    }
}

local bt_connected = false
local bt_ip = ""
local bt_device = ""
local last_reconnect = 0
local reconnect_timer = 0

function plugin:on_loaded()
    if not self.config.enabled then
        return
    end
    
    self:log("info", "bt-tether plugin loaded")
    
    -- Check if Bluetooth is available
    local handle = io.popen("which btmgmt 2>/dev/null")
    local result = handle:read("*a")
    handle:close()
    
    if result == "" then
        self:log("warn", "bluez tools not found, bt-tether disabled")
        return
    end
    
    -- Start monitoring Bluetooth
    self:start_monitoring()
end

function plugin:on_unload()
    self:stop_monitoring()
end

function plugin:start_monitoring()
    -- This would be called from the main loop in a real implementation
    -- For Lua plugin compatibility, we register callbacks
end

function plugin:on_internet_available()
    if not self.config.enabled then return end
    
    -- Internet is available, we can upload handshakes
    self:log("info", "Internet available via BT tether")
end

function plugin:on_epoch(epoch, status)
    if not self.config.enabled then return end
    
    -- Check Bluetooth connection status periodically
    if epoch % 5 == 0 then
        self:check_bt_status()
    end
    
    -- Attempt reconnect if needed
    if self.config.auto_reconnect and not bt_connected then
        local now = os.time()
        if now - last_reconnect >= self.config.reconnect_interval then
            self:attempt_reconnect()
            last_reconnect = now
        end
    end
end

function plugin:check_bt_status()
    -- Check if any PAN connection is active
    local handle = io.popen("ip link show bnep0 2>/dev/null | grep -c 'state UP'")
    local result = handle:read("*a")
    handle:close()
    
    local was_connected = bt_connected
    bt_connected = (result:match("%d+") or "0") ~= "0"
    
    if bt_connected and not was_connected then
        self:on_bt_connected()
    elseif not bt_connected and was_connected then
        self:on_bt_disconnected()
    end
    
    -- Get IP if connected
    if bt_connected then
        local handle2 = io.popen("ip -4 addr show bnep0 2>/dev/null | grep -oP 'inet \\K[\\d.]+'")
        local ip = handle2:read("*a"):gsub("%s+", "")
        handle2:close()
        if ip ~= "" then
            bt_ip = ip
        end
    end
end

function plugin:attempt_reconnect()
    if self.config.phone_mac ~= "" then
        self:log("info", "Attempting BT reconnect to " .. self.config.phone_mac)
        local cmd = string.format("bt-network -c %s nap", self.config.phone_mac)
        os.execute(cmd .. " >/dev/null 2>&1 &")
    else
        -- Scan for known devices
        self:scan_and_connect()
    end
end

function plugin:scan_and_connect()
    local handle = io.popen("bt-device -l 2>/dev/null | grep -E '^\\[.*\\] ' | head -10")
    local devices = handle:read("*a")
    handle:close()
    
    for line in devices:gmatch("[^\r\n]+") do
        local mac = line:match("%[([%x:]+)%]")
        local name = line:match("%]%s+(.+)")
        if mac then
            self:log("info", "Found BT device: " .. name .. " (" .. mac .. ")")
            -- Try to connect as PAN NAP
            local cmd = string.format("bt-network -c %s nap", mac)
            local result = os.execute(cmd .. " >/dev/null 2>&1")
            if result == 0 then
                bt_device = mac
                self.config.phone_mac = mac
                self:log("info", "Connected to " .. name)
                return
            end
        end
    end
end

function plugin:on_bt_connected()
    self:log("info", "Bluetooth PAN connected, IP: " .. bt_ip)
    bt_connected = true
end

function plugin:on_bt_disconnected()
    self:log("warn", "Bluetooth PAN disconnected")
    bt_connected = false
    bt_ip = ""
end

function plugin:on_ui_update(ui)
    if not self.config.enabled or not self.config.show_on_screen then return end
    
    if self.config.show_mini_status then
        local x, y = table.unpack(self.config.mini_status_position)
        local status = bt_connected and "BT: " .. bt_ip or "BT: --"
        ui:draw_text(x, y, status)
    end
    
    if self.config.show_detailed_status then
        local x, y = table.unpack(self.config.detailed_status_position)
        local status = bt_connected and ("BT Tether: " .. bt_ip .. " (" .. bt_device .. ")") or "BT Tether: Disconnected"
        ui:draw_text(x, y, status)
    end
end

function plugin:log(level, msg)
    print(string.format("[bt-tether] [%s] %s", level:upper(), msg))
end

return plugin
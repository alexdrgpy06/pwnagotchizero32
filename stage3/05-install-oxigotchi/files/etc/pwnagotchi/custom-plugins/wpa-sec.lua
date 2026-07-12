-- wpa-sec.lua - Upload handshakes to wpa-sec.stanev.org
-- Compatible with pwnagotchi plugin API

local plugin = {
    name = "wpa-sec",
    version = "1.0.0",
    author = "pwnagotchi-zero",
    description = "Upload handshakes to wpa-sec.stanev.org for distributed cracking",
    config = {
        enabled = true,
        api_key = "",
        api_url = "https://wpa-sec.stanev.org",
        download_results = true,
        show_pwd = false,
        single_files = false,
        upload_interval = 300,  -- 5 minutes
        max_retries = 3,
    }
}

local http = require("socket.http")
local ltn12 = require("ltn12")
local json = require("dkjson")
local upload_queue = {}
local last_upload = 0
local pending_results = {}

function plugin:on_loaded()
    if not self.config.enabled then
        return
    end
    
    if self.config.api_key == "" then
        self:log("warn", "wpa-sec API key not configured, plugin disabled")
        self.config.enabled = false
        return
    end
    
    self:log("info", "wpa-sec plugin loaded")
    
    -- Load any pending uploads from previous session
    self:load_queue()
end

function plugin:on_unload()
    self:save_queue()
end

function plugin:on_internet_available()
    if not self.config.enabled then return end
    self:process_queue()
end

function plugin:on_handshake_captured(path, ap, client)
    if not self.config.enabled then return end
    
    self:log("info", "Handshake captured: " .. path)
    
    -- Add to upload queue
    local entry = {
        path = path,
        ap = ap,
        client = client,
        timestamp = os.time(),
        retries = 0,
    }
    table.insert(upload_queue, entry)
    self:save_queue()
end

function plugin:on_epoch(epoch, status)
    if not self.config.enabled then return end
    
    local now = os.time()
    if now - last_upload >= self.config.upload_interval then
        self:process_queue()
        last_upload = now
    end
    
    -- Download cracked results periodically
    if self.config.download_results and epoch % 20 == 0 then
        self:download_results()
    end
end

function plugin:process_queue()
    if #upload_queue == 0 then return end
    
    self:log("info", "Processing upload queue (" .. #upload_queue .. " items)")
    
    local i = 1
    while i <= #upload_queue do
        local entry = upload_queue[i]
        
        if self:upload_handshake(entry) then
            self:log("info", "Uploaded: " .. entry.path)
            table.remove(upload_queue, i)
        else
            entry.retries = entry.retries + 1
            if entry.retries >= self.config.max_retries then
                self:log("error", "Max retries reached for: " .. entry.path)
                table.remove(upload_queue, i)
            else
                i = i + 1
            end
        end
    end
    
    self:save_queue()
end

function plugin:upload_handshake(entry)
    local file = io.open(entry.path, "rb")
    if not file then
        self:log("error", "Cannot open file: " .. entry.path)
        return false
    end
    
    local content = file:read("*a")
    file:close()
    
    -- Convert to hccapx if needed (using hcxpcapngtool)
    local hccapx_path = entry.path .. ".hccapx"
    local cmd = string.format("hcxpcapngtool -o %s %s 2>/dev/null", hccapx_path, entry.path)
    local result = os.execute(cmd)
    
    if result ~= 0 then
        self:log("warn", "Failed to convert to hccapx: " .. entry.path)
        -- Try uploading pcapng directly
        return self:upload_raw(entry.path, content, "pcapng")
    end
    
    local hccapx_file = io.open(hccapx_path, "rb")
    if not hccapx_file then
        return false
    end
    
    local hccapx_content = hccapx_file:read("*a")
    hccapx_file:close()
    
    local success = self:upload_raw(hccapx_path, hccapx_content, "hccapx")
    
    -- Clean up temp file
    os.remove(hccapx_path)
    
    return success
end

function plugin:upload_raw(filename, content, filetype)
    local boundary = "----pwnagotchi" .. os.time()
    local body = {}
    
    table.insert(body, "--" .. boundary)
    table.insert(body, 'Content-Disposition: form-data; name="file"; filename="' .. filename .. '"')
    table.insert(body, "Content-Type: application/octet-stream")
    table.insert(body, "")
    table.insert(body, content)
    table.insert(body, "--" .. boundary .. "--")
    table.insert(body, "")
    
    local request_body = table.concat(body, "\r\n")
    
    local response = {}
    local url = self.config.api_url .. "/?api&key=" .. self.config.api_key
    
    local res, code, headers = http.request{
        url = url,
        method = "POST",
        headers = {
            ["Content-Type"] = "multipart/form-data; boundary=" .. boundary,
            ["Content-Length"] = #request_body,
        },
        source = ltn12.source.string(request_body),
        sink = ltn12.sink.table(response),
    }
    
    if code == 200 then
        local resp_text = table.concat(response)
        if resp_text:match("success") or resp_text:match("ok") then
            return true
        end
        self:log("warn", "Upload response: " .. resp_text)
    else
        self:log("error", "Upload failed with code: " .. tostring(code))
    end
    
    return false
end

function plugin:download_results()
    if self.config.api_key == "" then return end
    
    local url = self.config.api_url .. "/?api&key=" .. self.config.api_key .. "&dl=1"
    
    local response = {}
    local res, code = http.request{
        url = url,
        sink = ltn12.sink.table(response),
    }
    
    if code == 200 then
        local content = table.concat(response)
        -- Save potfile
        local potfile = "/etc/pwnagotchi/wpa-sec.potfile"
        local f = io.open(potfile, "w")
        if f then
            f:write(content)
            f:close()
            self:log("info", "Downloaded cracked passwords to " .. potfile)
        end
    end
end

function plugin:save_queue()
    local queue_file = "/etc/pwnagotchi/wpa-sec-queue.json"
    local f = io.open(queue_file, "w")
    if f then
        f:write(json.encode(upload_queue))
        f:close()
    end
end

function plugin:load_queue()
    local queue_file = "/etc/pwnagotchi/wpa-sec-queue.json"
    local f = io.open(queue_file, "r")
    if f then
        local content = f:read("*a")
        f:close()
        local ok, data = pcall(json.decode, content)
        if ok and data then
            upload_queue = data
            self:log("info", "Loaded " .. #upload_queue .. " pending uploads")
        end
    end
end

function plugin:on_ui_update(ui)
    if not self.config.enabled then return end
    
    local status = #upload_queue > 0 and ("WPA-SEC: " .. #upload_queue .. " pending") or "WPA-SEC: idle"
    ui:draw_text(0, 100, status)
end

function plugin:log(level, msg)
    print(string.format("[wpa-sec] [%s] %s", level:upper(), msg))
end

return plugin
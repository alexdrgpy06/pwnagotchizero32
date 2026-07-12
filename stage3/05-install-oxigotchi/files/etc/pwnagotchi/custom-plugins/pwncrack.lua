-- pwncrack.lua - Upload handshakes to pwncrack.org
-- Compatible with pwnagotchi plugin API

local plugin = {
    name = "pwncrack",
    version = "1.0.0",
    author = "pwnagotchi-zero",
    description = "Upload handshakes to pwncrack.org for cloud cracking",
    config = {
        enabled = true,
        key = "",
        api_url = "https://api.pwncrack.org",
        upload_interval = 300,
        max_retries = 3,
    }
}

local upload_queue = {}
local last_upload = 0

function plugin:on_loaded()
    if not self.config.enabled then return end
    
    if self.config.key == "" then
        self:log("warn", "pwncrack API key not configured, plugin disabled")
        self.config.enabled = false
        return
    end
    
    self:log("info", "pwncrack plugin loaded")
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
end

function plugin:process_queue()
    if #upload_queue == 0 then return end
    
    self:log("info", "Processing pwncrack queue (" .. #upload_queue .. " items)")
    
    local i = 1
    while i <= #upload_queue do
        local entry = upload_queue[i]
        
        if self:upload_handshake(entry) then
            self:log("info", "Uploaded to pwncrack: " .. entry.path)
            table.remove(upload_queue, i)
        else
            entry.retries = entry.retries + 1
            if entry.retries >= self.config.max_retries then
                self:log("error", "Max retries for: " .. entry.path)
                table.remove(upload_queue, i)
            else
                i = i + 1
            end
        end
    end
    
    self:save_queue()
end

function plugin:upload_handshake(entry)
    -- Convert to hccapx
    local hccapx_path = entry.path .. ".hccapx"
    local cmd = string.format("hcxpcapngtool -o %s %s 2>/dev/null", hccapx_path, entry.path)
    local result = os.execute(cmd)
    
    if result ~= 0 then
        self:log("warn", "hccapx conversion failed: " .. entry.path)
        return false
    end
    
    local file = io.open(hccapx_path, "rb")
    if not file then return false end
    
    local content = file:read("*a")
    file:close()
    
    -- Upload via HTTP POST
    local boundary = "----pwncrack" .. os.time()
    local body = {}
    
    table.insert(body, "--" .. boundary)
    table.insert(body, 'Content-Disposition: form-data; name="file"; filename="' .. entry.path .. '"')
    table.insert(body, "Content-Type: application/octet-stream")
    table.insert(body, "")
    table.insert(body, content)
    table.insert(body, "--" .. boundary)
    table.insert(body, 'Content-Disposition: form-data; name="apikey"')
    table.insert(body, "")
    table.insert(body, self.config.key)
    table.insert(body, "--" .. boundary .. "--")
    table.insert(body, "")
    
    local request_body = table.concat(body, "\r\n")
    
    local response = {}
    local http = require("socket.http")
    local ltn12 = require("ltn12")
    
    local res, code = http.request{
        url = self.config.api_url .. "/upload",
        method = "POST",
        headers = {
            ["Content-Type"] = "multipart/form-data; boundary=" .. boundary,
            ["Content-Length"] = #request_body,
        },
        source = ltn12.source.string(request_body),
        sink = ltn12.sink.table(response),
    }
    
    os.remove(hccapx_path)
    
    if code == 200 then
        local resp_text = table.concat(response)
        if resp_text:match("success") or resp_text:match("ok") then
            return true
        end
        self:log("warn", "pwncrack response: " .. resp_text)
    else
        self:log("error", "pwncrack upload failed: " .. tostring(code))
    end
    
    return false
end

function plugin:save_queue()
    local queue_file = "/etc/pwnagotchi/pwncrack-queue.json"
    local f = io.open(queue_file, "w")
    if f then
        local json = require("dkjson")
        f:write(json.encode(upload_queue))
        f:close()
    end
end

function plugin:load_queue()
    local queue_file = "/etc/pwnagotchi/pwncrack-queue.json"
    local f = io.open(queue_file, "r")
    if f then
        local content = f:read("*a")
        f:close()
        local json = require("dkjson")
        local ok, data = pcall(json.decode, content)
        if ok and data then
            upload_queue = data
            self:log("info", "Loaded " .. #upload_queue .. " pending pwncrack uploads")
        end
    end
end

function plugin:on_ui_update(ui)
    if not self.config.enabled then return end
    
    local status = #upload_queue > 0 and ("PWNCRACK: " .. #upload_queue .. " pending") or "PWNCRACK: idle"
    ui:draw_text(0, 110, status)
end

function plugin:log(level, msg)
    print(string.format("[pwncrack] [%s] %s", level:upper(), msg))
end

return plugin
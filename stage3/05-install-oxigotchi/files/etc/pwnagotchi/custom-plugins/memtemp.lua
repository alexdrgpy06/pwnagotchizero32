-- memtemp.lua - Memory and temperature monitoring
-- Compatible with pwnagotchi plugin API

local plugin = {
    name = "memtemp",
    version = "1.0.0",
    author = "pwnagotchi-zero",
    description = "Display memory usage and CPU temperature",
    config = {
        enabled = true,
        scale = "celsius",  -- celsius or fahrenheit
        orientation = "horizontal",  -- horizontal or vertical
        update_interval = 5,  -- seconds
        show_memory = true,
        show_temperature = true,
        show_cpu = true,
    }
}

local last_update = 0

function plugin:on_loaded()
    if not self.config.enabled then return end
    self:log("info", "memtemp plugin loaded")
end

function plugin:on_epoch(epoch, status)
    if not self.config.enabled then return end
    
    local now = os.time()
    if now - last_update >= self.config.update_interval then
        self:update_stats()
        last_update = now
    end
end

function plugin:update_stats()
    -- Read memory info
    local mem_info = {}
    local f = io.open("/proc/meminfo", "r")
    if f then
        for line in f:lines() do
            local k, v = line:match("([^:]+):%s+(%d+)")
            if k and v then
                mem_info[k] = tonumber(v)
            end
        end
        f:close()
    end
    
    self.mem_total = mem_info.MemTotal or 0
    self.mem_available = mem_info.MemAvailable or 0
    self.mem_used = self.mem_total - self.mem_available
    self.mem_percent = self.mem_total > 0 and math.floor((self.mem_used / self.mem_total) * 100) or 0
    
    -- Read CPU temperature
    local temp = 0
    local temp_files = {
        "/sys/class/thermal/thermal_zone0/temp",
        "/sys/class/thermal/thermal_zone1/temp",
    }
    
    for _, file in ipairs(temp_files) do
        local f = io.open(file, "r")
        if f then
            local t = f:read("*n")
            f:close()
            if t and t > temp then temp = t end
        end
    end
    
    self.temp_c = temp / 1000
    self.temp_f = (self.temp_c * 9/5) + 32
    
    -- Read CPU load
    local load = ""
    local f = io.open("/proc/loadavg", "r")
    if f then
        load = f:read("*l")
        f:close()
    end
    self.load_1m = load:match("^([%d.]+)")
end

function plugin:on_ui_update(ui)
    if not self.config.enabled then return end
    
    local y = 110
    local x = 0
    
    if self.config.orientation == "vertical" then
        x = 200
        y = 0
    end
    
    local texts = {}
    
    if self.config.show_memory then
        table.insert(texts, string.format("MEM: %d%% (%d/%d MB)", 
            self.mem_percent,
            math.floor(self.mem_used / 1024),
            math.floor(self.mem_total / 1024)))
    end
    
    if self.config.show_temperature then
        local temp = self.config.scale == "fahrenheit" and self.temp_f or self.temp_c
        local unit = self.config.scale == "fahrenheit" and "F" or "C"
        table.insert(texts, string.format("CPU: %.1f%s", temp, unit))
    end
    
    if self.config.show_cpu and self.load_1m then
        table.insert(texts, string.format("LOAD: %s", self.load_1m))
    end
    
    for i, text in ipairs(texts) do
        local ty = y + (i - 1) * 10
        if self.config.orientation == "vertical" then
            ui:draw_text(x, ty, text)
        else
            ui:draw_text(x, ty, text)
        end
    end
end

function plugin:log(level, msg)
    print(string.format("[memtemp] [%s] %s", level:upper(), msg))
end

return plugin
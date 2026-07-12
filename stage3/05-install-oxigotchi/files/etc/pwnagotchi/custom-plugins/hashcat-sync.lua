-- hashcat-sync.lua - Sync handshakes to local hashcat cracking service
-- Compatible with pwnagotchi plugin API

local plugin = {
    name = "hashcat-sync",
    version = "1.0.0",
    author = "pwnagotchi-zero",
    description = "Sync handshakes to local hashcat service via rsync/SSH",
    config = {
        enabled = true,
        remote_host = "hashcat.local",
        remote_user = "hashcat",
        remote_path = "/home/hashcat/captures",
        local_path = "/etc/pwnagotchi/handshakes",
        sync_interval = 300,  -- 5 minutes
        pull_potfile = true,
        potfile_path = "/home/hashcat/hashcat.potfile",
        local_potfile = "/etc/pwnagotchi/hashcat.potfile",
        ssh_key = "/home/pi/.ssh/id_ed25519",
        ssh_port = 22,
        rsync_options = "-avz --progress",
        convert_to_hccapx = true,
        delete_after_sync = false,
    }
}

local last_sync = 0
local last_pull = 0

function plugin:on_loaded()
    if not self.config.enabled then return end
    
    self:log("info", "hashcat-sync plugin loaded")
    self:check_ssh_key()
end

function plugin:check_ssh_key()
    local key_path = self.config.ssh_key
    local f = io.open(key_path, "r")
    if not f then
        self:log("warn", "SSH key not found at " .. key_path .. ", generating...")
        os.execute("mkdir -p /home/pi/.ssh && ssh-keygen -t ed25519 -f " .. key_path .. " -N '' -q")
        self:log("info", "SSH key generated. Add public key to " .. self.config.remote_user .. "@" .. self.config.remote_host .. ":~/.ssh/authorized_keys")
    else
        f:close()
    end
end

function plugin:on_internet_available()
    if not self.config.enabled then return end
    -- Local network sync doesn't require internet, but we can use this trigger
    self:attempt_sync()
end

function plugin:on_handshake_captured(path, ap, client)
    if not self.config.enabled then return end
    
    -- Optionally convert to hccapx immediately
    if self.config.convert_to_hccapx then
        local hccapx_path = path .. ".hccapx"
        local cmd = string.format("hcxpcapngtool -o %s %s 2>/dev/null", hccapx_path, path)
        local result = os.execute(cmd)
        if result == 0 then
            self:log("info", "Converted to hccapx: " .. hccapx_path)
        end
    end
end

function plugin:on_epoch(epoch, status)
    if not self.config.enabled then return end
    
    local now = os.time()
    
    -- Push captures
    if now - last_sync >= self.config.sync_interval then
        self:attempt_sync()
        last_sync = now
    end
    
    -- Pull potfile (less frequent)
    if self.config.pull_potfile and now - last_pull >= self.config.sync_interval * 4 then
        self:pull_potfile()
        last_pull = now
    end
end

function plugin:attempt_sync()
    self:log("info", "Syncing handshakes to " .. self.config.remote_host)
    
    -- Build rsync command
    local ssh_cmd = string.format("ssh -i %s -p %d -o StrictHostKeyChecking=no -o ConnectTimeout=10", 
        self.config.ssh_key, self.config.ssh_port)
    
    local cmd = string.format("rsync %s -e \"%s\" %s/ %s@%s:%s/ 2>&1",
        self.config.rsync_options,
        ssh_cmd,
        self.config.local_path,
        self.config.remote_user,
        self.config.remote_host,
        self.config.remote_path
    )
    
    local handle = io.popen(cmd)
    local output = handle:read("*a")
    local success = handle:close()
    
    if success then
        self:log("info", "Sync successful")
        
        -- Optionally delete local files after sync
        if self.config.delete_after_sync then
            os.execute(string.format("find %s -name '*.pcapng' -mtime +1 -delete", self.config.local_path))
            os.execute(string.format("find %s -name '*.hccapx' -mtime +1 -delete", self.config.local_path))
        end
    else
        self:log("warn", "Sync failed: " .. output)
    end
end

function plugin:pull_potfile()
    self:log("info", "Pulling potfile from " .. self.config.remote_host)
    
    local ssh_cmd = string.format("ssh -i %s -p %d -o StrictHostKeyChecking=no -o ConnectTimeout=10", 
        self.config.ssh_key, self.config.ssh_port)
    
    local cmd = string.format("scp -i %s -P %d -o StrictHostKeyChecking=no %s@%s:%s %s 2>&1",
        self.config.ssh_key,
        self.config.ssh_port,
        self.config.remote_user,
        self.config.remote_host,
        self.config.potfile_path,
        self.config.local_potfile
    )
    
    local handle = io.popen(cmd)
    local output = handle:read("*a")
    local success = handle:close()
    
    if success then
        self:log("info", "Potfile updated")
    else
        self:log("warn", "Potfile pull failed: " .. output)
    end
end

function plugin:on_ui_update(ui)
    if not self.config.enabled then return end
    
    local status = "HASHCAT: "
    local f = io.open(self.config.local_potfile, "r")
    if f then
        local count = 0
        for _ in f:lines() do count = count + 1 end
        f:close()
        status = status .. count .. " cracked"
    else
        status = status .. "no potfile"
    end
    ui:draw_text(0, 120, status)
end

function plugin:log(level, msg)
    print(string.format("[hashcat-sync] [%s] %s", level:upper(), msg))
end

return plugin
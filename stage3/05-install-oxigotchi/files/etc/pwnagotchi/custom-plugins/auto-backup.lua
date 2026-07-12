-- auto-backup.lua - Auto backup config and handshakes
-- Compatible with pwnagotchi plugin API

local plugin = {
    name = "auto-backup",
    version = "1.0.0",
    author = "pwnagotchi-zero",
    description = "Automatic backup of config and handshakes",
    config = {
        enabled = true,
        backup_location = "/etc/pwnagotchi/backups",
        max_backups = 10,
        backup_interval = 3600,  -- 1 hour
        include_handshakes = true,
        include_config = true,
        include_logs = false,
    }
}

local last_backup = 0

function plugin:on_loaded()
    if not self.config.enabled then return end
    
    -- Ensure backup directory exists
    os.execute("mkdir -p " .. self.config.backup_location)
    
    self:log("info", "auto-backup plugin loaded")
end

function plugin:on_epoch(epoch, status)
    if not self.config.enabled then return end
    
    local now = os.time()
    if now - last_backup >= self.config.backup_interval then
        self:create_backup()
        last_backup = now
    end
end

function plugin:on_internet_available()
    -- Could push backups to remote here
end

function plugin:create_backup()
    self:log("info", "Creating backup...")
    
    local timestamp = os.date("%Y%m%d-%H%M%S")
    local backup_name = "pwnagotchi-backup-" .. timestamp
    local backup_dir = self.config.backup_location .. "/" .. backup_name
    
    os.execute("mkdir -p " .. backup_dir)
    
    local files_to_backup = {}
    
    if self.config.include_config then
        table.insert(files_to_backup, "/etc/pwnagotchi/config.toml")
        table.insert(files_to_backup, "/etc/pwnagotchi/conf.d/")
    end
    
    if self.config.include_handshakes then
        table.insert(files_to_backup, "/etc/pwnagotchi/handshakes/")
    end
    
    if self.config.include_logs then
        table.insert(files_to_backup, "/etc/pwnagotchi/log/")
    end
    
    for _, path in ipairs(files_to_backup) do
        local cmd = string.format("cp -r %s %s/ 2>/dev/null", path, backup_dir)
        os.execute(cmd)
    end
    
    -- Create tarball
    local tar_cmd = string.format("cd %s && tar -czf %s.tar.gz %s && rm -rf %s",
        self.config.backup_location,
        backup_name,
        backup_name,
        backup_name)
    os.execute(tar_cmd)
    
    -- Clean old backups
    self:cleanup_old_backups()
    
    self:log("info", "Backup created: " .. backup_name .. ".tar.gz")
end

function plugin:cleanup_old_backups()
    local cmd = string.format("ls -t %s/pwnagotchi-backup-*.tar.gz 2>/dev/null | tail -n +%d | xargs -r rm -f",
        self.config.backup_location,
        self.config.max_backups + 1)
    os.execute(cmd)
end

function plugin:on_ui_update(ui)
    if not self.config.enabled then return end
    
    -- Show backup status
    local handle = io.popen("ls -t " .. self.config.backup_location .. "/pwnagotchi-backup-*.tar.gz 2>/dev/null | head -1")
    local latest = handle:read("*a"):gsub("%s+", "")
    handle:close()
    
    if latest ~= "" then
        local name = latest:match("([^/]+)$")
        ui:draw_text(0, 130, "BACKUP: " .. name)
    else
        ui:draw_text(0, 130, "BACKUP: none")
    end
end

function plugin:log(level, msg)
    print(string.format("[auto-backup] [%s] %s", level:upper(), msg))
end

return plugin
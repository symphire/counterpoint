
-- Lua: if key missing -> {-1,0}
-- If match -> delete, {1,tries}
-- If mismatch -> decrement tries or delete when hits 0, {0,remaining}

local key = KEYS[1]
local provided = ARGV[1]

if redis.call('EXISTS', key) == 0 then
    return {-1, 0}
end

local h = redis.call('HGET', key, 'h')
local tr = tonumber(redis.call('HGET', key, 'tries')) or 0

if h == provided then
    redis.call('DEL', key)
    return {1, tr}
else
    if tr <= 1 then
        redis.call('DEL', key)
        return {0, 0}
    else
        local newtr = redis.call('HINCRBY', key, 'tries', -1)
        return {0, tonumber(newtr)}
    end
end

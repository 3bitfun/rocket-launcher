package com.rocketlauncher.companion;

import static com.rocketlauncher.companion.RocketCompanion.text;

import net.minecraft.client.Minecraft;
import net.minecraft.network.chat.Component;

enum HudModule {
    FPS("fps", true, 0, 0),
    COORDINATES("coordinates", false, 0, 1),
    HEADING("heading", false, 1, 0),
    CLOCK("clock", false, 1, 1),
    PING("ping", false, 0, 0.18),
    MEMORY("memory", false, 1, 0.18),
    KEYS("keys", false, 0, 0.36),
    ARMOR("armor", false, 1, 0.36);

    final String id;
    final boolean enabled;
    final ClientSettings.Position position;

    HudModule(String id, boolean enabled, double x, double y) {
        this.id = id;
        this.enabled = enabled;
        this.position = new ClientSettings.Position(x, y);
    }

    Component displayName() {
        return text("module." + id);
    }

    Component value(Minecraft client) {
        var player = client.player;
        return switch (this) {
            case FPS -> text("value.fps", ClientPlatform.fps(client));
            case COORDINATES -> {
                var block = player == null ? net.minecraft.core.BlockPos.ZERO : player.blockPosition();
                yield text("value.coordinates", block.getX(), block.getY(), block.getZ());
            }
            case HEADING -> {
                String direction = player == null ? "north" : player.getDirection().getName();
                yield text("value.heading." + direction);
            }
            case CLOCK -> {
                long ticks = client.level == null ? 0 : Math.floorMod(client.level.getDayTime(), 24000);
                long hours = (ticks / 1000 + 6) % 24;
                long minutes = ticks % 1000 * 60 / 1000;
                yield text("value.clock", twoDigits(hours), twoDigits(minutes));
            }
            case PING -> {
                var connection = client.getConnection();
                var info = player == null || connection == null ? null : connection.getPlayerInfo(player.getUUID());
                yield info == null || client.isLocalServer()
                    ? text("value.ping_unavailable")
                    : text("value.ping", info.getLatency());
            }
            case MEMORY -> {
                Runtime runtime = Runtime.getRuntime();
                long usedMiB = (runtime.totalMemory() - runtime.freeMemory()) / (1024 * 1024);
                long limitMiB = runtime.maxMemory() / (1024 * 1024);
                yield text("value.memory", usedMiB, limitMiB);
            }
            case KEYS -> text("value.keys",
                pressed(client.options.keyUp),
                pressed(client.options.keyLeft),
                pressed(client.options.keyDown),
                pressed(client.options.keyRight),
                pressed(client.options.keyJump));
            case ARMOR -> text("value.armor", player == null ? 0 : player.getArmorValue());
        };
    }

    private static String twoDigits(long number) {
        return (number < 10 ? "0" : "") + number;
    }

    private static String pressed(net.minecraft.client.KeyMapping mapping) {
        String key = mapping.getTranslatedKeyMessage().getString();
        return mapping.isDown() ? "[" + key + "]" : key;
    }
}

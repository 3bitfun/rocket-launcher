package com.rocketlauncher.companion;

import java.io.IOException;
import java.io.RandomAccessFile;
import java.net.ServerSocket;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.regex.Pattern;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.client.Minecraft;

final class WorldSharing {
    private static final Pattern RELAY_ADDRESS = Pattern.compile(
        "Domain assigned: ([a-z0-9-]{1,63}(?:\\.[a-z0-9-]{1,63})*\\.e4mc\\.link)\\s*$",
        Pattern.CASE_INSENSITIVE | Pattern.MULTILINE);
    private final boolean installed = FabricLoader.getInstance().isModLoaded("e4mc")
        || FabricLoader.getInstance().isModLoaded("e4mc_minecraft");
    private Object world;
    private Object relay;
    private String state = "";
    private String address;
    private long logStart;

    boolean installed() { return installed; }
    boolean connecting() { return "STARTING".equals(state); }
    boolean sharing() { return "STARTED".equals(state); }
    boolean failed() { return "UNHEALTHY".equals(state); }
    String address() { return sharing() ? address : null; }

    void poll(Minecraft client) {
        if (!installed) return;
        Object currentWorld = client.getSingleplayerServer();
        Path log = client.gameDirectory.toPath().resolve("logs").resolve("latest.log");
        if (world != currentWorld) {
            world = currentWorld;
            relay = null;
            address = null;
            logStart = logSize(log);
        }
        Object next = null;
        String nextState = "";
        if (world != null) {
            try {
                next = Class.forName("link.e4mc.E4mcClient").getField("session").get(null);
                if (next != null && next.getClass().getField("state").get(next) instanceof Enum<?> value) {
                    nextState = value.name();
                }
            } catch (ReflectiveOperationException | LinkageError ignored) {
                // Older e4mc builds still expose the address and stop action in their own chat messages.
            }
        }
        if (relay != next || !nextState.equals("STARTED")) address = null;
        if (next == null || nextState.equals("STOPPED") || nextState.equals("STOPPING")) logStart = logSize(log);
        relay = next;
        state = nextState;
        if (sharing() && address == null) address = readAddress(log, logStart);
    }

    boolean stop() {
        if (relay == null || (!sharing() && !connecting())) return false;
        try {
            relay.getClass().getMethod("stop").invoke(relay);
            state = "STOPPING";
            address = null;
            return true;
        } catch (ReflectiveOperationException | LinkageError ignored) {
            return false;
        }
    }

    private static long logSize(Path log) {
        try { return Files.size(log); } catch (IOException ignored) { return 0; }
    }

    private static String readAddress(Path log, long start) {
        try (var file = new RandomAccessFile(log.toFile(), "r")) {
            long length = file.length();
            if (start > length) start = 0;
            long offset = Math.max(start, length - 64 * 1024);
            file.seek(offset);
            byte[] bytes = new byte[(int) (length - offset)];
            file.readFully(bytes);
            var match = RELAY_ADDRESS.matcher(new String(bytes, StandardCharsets.UTF_8));
            String latest = null;
            while (match.find()) latest = match.group(1);
            return latest;
        } catch (IOException ignored) {
            return null;
        }
    }

    static int availablePort() throws IOException {
        try (var socket = new ServerSocket(0)) { return socket.getLocalPort(); }
    }
}

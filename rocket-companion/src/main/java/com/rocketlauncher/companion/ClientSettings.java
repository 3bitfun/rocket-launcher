package com.rocketlauncher.companion;

import java.io.IOException;
import java.nio.channels.FileChannel;
import java.nio.file.AtomicMoveNotSupportedException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.nio.file.StandardOpenOption;
import java.nio.file.attribute.BasicFileAttributes;
import java.util.Objects;
import java.util.Properties;
import java.util.function.Consumer;
import net.fabricmc.loader.api.FabricLoader;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

final class ClientSettings {
    private static final Logger LOGGER = LoggerFactory.getLogger("Rocket Launcher Client");
    private final Path local = FabricLoader.getInstance().getConfigDir().resolve("rocket-client.properties");
    private final String shared = System.getProperty("rocketlauncher.clientConfig", "");
    private final Path path = shared.isBlank() ? local : Path.of(shared);
    private Properties values = new Properties();
    private CrosshairStyle crosshair = CrosshairStyle.DEFAULT;
    private Stamp stamp;
    private boolean saveFailed;

    record Position(double x, double y) {}
    private record Stamp(Object key, long length, java.nio.file.attribute.FileTime modified) {}

    void initialize() {
        refresh();
        if (shared() && stamp == null && !values.isEmpty()) {
            update(next -> { });
        }
    }

    boolean shared() { return !shared.isBlank(); }
    boolean saveFailed() { return saveFailed; }

    CrosshairStyle crosshair() { return crosshair; }

    void setCrosshair(CrosshairStyle style) {
        update(next -> next.setProperty("crosshair", style.encode()));
    }

    boolean hasCrosshairPreset(int slot) { return values.containsKey("crosshair.preset." + slot); }

    void loadCrosshairPreset(int slot) {
        if (hasCrosshairPreset(slot)) setCrosshair(CrosshairStyle.decode(values.getProperty("crosshair.preset." + slot)));
    }

    void saveCrosshairPreset(int slot) {
        update(next -> next.setProperty("crosshair.preset." + slot, crosshair.encode()));
    }

    boolean enabled(HudModule module) {
        return Boolean.parseBoolean(values.getProperty(module.id + ".enabled", Boolean.toString(module.enabled)));
    }

    Position position(HudModule module) {
        return new Position(number(module.id + ".x", module.position.x()),
            number(module.id + ".y", module.position.y()));
    }

    void setEnabled(HudModule module, boolean enabled) {
        update(next -> next.setProperty(module.id + ".enabled", Boolean.toString(enabled)));
    }

    void setPosition(HudModule module, Position position) {
        update(next -> {
            next.setProperty(module.id + ".x", Double.toString(position.x()));
            next.setProperty(module.id + ".y", Double.toString(position.y()));
        });
    }

    void resetPositions() {
        update(next -> {
            for (HudModule module : HudModule.values()) {
                next.remove(module.id + ".x");
                next.remove(module.id + ".y");
            }
        });
    }

    void refresh() {
        try {
            Stamp current = readStamp();
            if (current == null || !Objects.equals(current, stamp)) {
                values = load();
                crosshair = CrosshairStyle.decode(values.getProperty("crosshair"));
                stamp = current;
            }
        } catch (IOException | IllegalArgumentException error) {
            LOGGER.warn("Could not read Rocket Client settings", error);
        }
    }

    private double number(String key, double fallback) {
        String value = values.getProperty(key);
        if (value != null) {
            try {
                double number = Double.parseDouble(value);
                if (Double.isFinite(number)) return clamp(number);
            } catch (NumberFormatException ignored) {
                // Use the default for manually edited settings with invalid coordinates.
            }
        }
        return fallback;
    }

    static double clamp(double value) {
        return Math.max(0, Math.min(1, value));
    }

    private Stamp readStamp() throws IOException {
        if (!Files.exists(path)) return null;
        BasicFileAttributes attributes = Files.readAttributes(path, BasicFileAttributes.class);
        return new Stamp(attributes.fileKey(), attributes.size(), attributes.lastModifiedTime());
    }

    private Properties load() throws IOException {
        Properties result = new Properties();
        Path source = Files.exists(path) ? path : local;
        if (Files.exists(source)) {
            try (var input = Files.newInputStream(source)) {
                result.load(input);
            }
            return result;
        }
        Path legacy = local.resolveSibling("rocket-client-hud.txt");
        if (Files.exists(legacy)) {
            String enabled = Boolean.toString(Boolean.parseBoolean(Files.readString(legacy).trim()));
            result.setProperty("fps.enabled", enabled);
            result.setProperty("coordinates.enabled", enabled);
        }
        return result;
    }

    private void update(Consumer<Properties> edit) {
        try {
            Files.createDirectories(path.getParent());
            Path lockPath = path.resolveSibling(path.getFileName() + ".lock");
            try (var channel = FileChannel.open(lockPath, StandardOpenOption.CREATE, StandardOpenOption.WRITE);
                 var lock = channel.lock()) {
                Properties next = load();
                edit.accept(next);
                Path staged = Files.createTempFile(path.getParent(), "rocket-client-", ".tmp");
                try {
                    try (var output = Files.newOutputStream(staged)) {
                        next.store(output, "Rocket Client HUD");
                    }
                    try {
                        Files.move(staged, path, StandardCopyOption.ATOMIC_MOVE, StandardCopyOption.REPLACE_EXISTING);
                    } catch (AtomicMoveNotSupportedException error) {
                        Files.move(staged, path, StandardCopyOption.REPLACE_EXISTING);
                    }
                } finally {
                    Files.deleteIfExists(staged);
                }
                values = next;
                crosshair = CrosshairStyle.decode(values.getProperty("crosshair"));
                stamp = readStamp();
                saveFailed = false;
            }
        } catch (IOException | IllegalArgumentException error) {
            saveFailed = true;
            LOGGER.warn("Could not save Rocket Client settings", error);
        }
    }
}

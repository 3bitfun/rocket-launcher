package com.rocketlauncher.companion;

import static com.rocketlauncher.companion.RocketCompanion.text;
import static com.rocketlauncher.companion.RocketStyle.*;

import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.network.chat.Component;

final class WorldShareScreen extends Screen {
    private final Screen parent;
    private final WorldSharing sharing;
    private Control start, copy, stop;
    private Component message;
    private int left, top, span;

    WorldShareScreen(Screen parent, WorldSharing sharing) {
        super(text("sharing.title"));
        this.parent = parent;
        this.sharing = sharing;
    }

    @Override
    protected void init() {
        span = Math.min(width - 16, 395);
        left = (width - span) / 2;
        top = Math.max(5, (height - 156) / 2);
        int size = (span - 26) / 3;
        addRenderableWidget(new Control(text("back"), pressed -> onClose(), left + span - 62, top + 6, 54));
        start = addRenderableWidget(new Control(text("sharing.start"), pressed -> {
            message = null;
            if (!ClientPlatform.openLan(this, minecraft)) message = text("sharing.start_failed");
        }, left + 8, top + 111, size));
        copy = addRenderableWidget(new Control(text("sharing.copy"), pressed -> {
            String address = sharing.address();
            if (address != null) {
                minecraft.keyboardHandler.setClipboard(address);
                message = text("sharing.copied");
            }
        }, left + 13 + size, top + 111, size));
        stop = addRenderableWidget(new Control(text("sharing.stop"), pressed -> {
            message = text(sharing.stop() ? "sharing.stopped" : "sharing.stop_failed");
        }, left + 18 + size * 2, top + 111, size));
        tick();
    }

    @Override
    public void tick() {
        var world = minecraft.getSingleplayerServer();
        start.active = world != null && !world.isPublished();
        copy.active = sharing.address() != null;
        stop.active = sharing.sharing() || sharing.connecting();
    }

    @Override
    public void renderBackground(GuiGraphics graphics, int mouseX, int mouseY, float delta) {}

    @Override
    public void render(GuiGraphics graphics, int mouseX, int mouseY, float delta) {
        graphics.fill(0, 0, width, height, 0xB0090B10);
        frame(graphics, left, top, span, 156, BORDER, PANEL);
        header(graphics, font, title, left, top, span, 63);
        Component status = sharing.sharing() ? text("sharing.active")
            : sharing.connecting() ? text("sharing.connecting")
            : sharing.failed() ? text("sharing.failed") : text("sharing.ready");
        line(graphics, font, status, left + 10, top + 43, span - 20, sharing.sharing() ? ACCENT : TEXT);
        String address = sharing.address();
        line(graphics, font, address != null ? text("sharing.address", address) : text("sharing.chat"),
            left + 10, top + 59, span - 20, MUTED);
        line(graphics, font, text("sharing.public"), left + 10, top + 75, span - 20, MUTED);
        line(graphics, font, text("sharing.lifecycle"), left + 10, top + 91, span - 20, MUTED);
        if (message != null) line(graphics, font, message, left + 10, top + 139, span - 20, TEXT);
        super.render(graphics, mouseX, mouseY, delta);
    }

    @Override
    public void onClose() { minecraft.setScreen(parent); }
}

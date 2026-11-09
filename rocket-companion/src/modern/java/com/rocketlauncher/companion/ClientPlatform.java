package com.rocketlauncher.companion;

import com.mojang.blaze3d.platform.InputConstants;
import java.util.function.Consumer;
import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
import net.fabricmc.fabric.api.client.rendering.v1.hud.HudElementRegistry;
import net.fabricmc.fabric.api.client.rendering.v1.hud.VanillaHudElements;
import net.minecraft.client.KeyMapping;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.options.OptionsScreen;
import net.minecraft.client.input.KeyEvent;
import net.minecraft.client.input.MouseButtonEvent;
import net.minecraft.network.chat.Component;
import net.minecraft.resources.@IDENTIFIER@;

final class ClientPlatform {
    static KeyMapping registerMenuKey() {
        var category = KeyMapping.Category.register(@IDENTIFIER@.fromNamespaceAndPath("rocket_companion", "client"));
        return KeyBindingHelper.registerKeyBinding(new KeyMapping("key.rocket_companion.menu",
            InputConstants.UNKNOWN.getValue(), category));
    }

    static void registerHud(Consumer<GuiGraphics> render) {
        HudElementRegistry.attachElementBefore(VanillaHudElements.CHAT,
            @IDENTIFIER@.fromNamespaceAndPath("rocket_companion", "hud"), (graphics, delta) -> render.accept(graphics));
    }

    static boolean debugVisible(Minecraft client) { return client.getDebugOverlay().showDebugScreen(); }

    static boolean openLan(Screen parent, Minecraft client) {
        if (client.getSingleplayerServer() == null || client.getSingleplayerServer().isPublished()) return false;
        client.setScreen(new net.minecraft.client.gui.screens.ShareToLanScreen(parent));
        return true;
    }

    static void registerCrosshair() {
        HudElementRegistry.replaceElement(VanillaHudElements.CROSSHAIR, vanilla -> (graphics, delta) -> {
            if (!CrosshairRenderer.renderInGame(graphics)) vanilla.render(graphics, delta);
        });
    }

    static int fps(Minecraft client) { return client.getFps(); }

    static void openOptions(Screen parent, Minecraft client) {
        client.setScreen(new OptionsScreen(parent, client.options));
    }

    abstract static class PointerScreen extends Screen {
        PointerScreen(Component title) { super(title); }

        abstract boolean grab(double x, double y, int button);
        abstract boolean drag(double x, double y, int button);
        abstract boolean drop(int button);
        abstract boolean moveSelection(int key);

        @Override
        public boolean keyPressed(KeyEvent event) {
            return moveSelection(event.key()) || super.keyPressed(event);
        }

        @Override
        public boolean mouseClicked(MouseButtonEvent event, boolean doubleClick) {
            return grab(event.x(), event.y(), event.button()) || super.mouseClicked(event, doubleClick);
        }

        @Override
        public boolean mouseDragged(MouseButtonEvent event, double dx, double dy) {
            return drag(event.x(), event.y(), event.button()) || super.mouseDragged(event, dx, dy);
        }

        @Override
        public boolean mouseReleased(MouseButtonEvent event) {
            return drop(event.button()) || super.mouseReleased(event);
        }
    }
}

package com.rocketlauncher.companion;

import com.mojang.blaze3d.platform.InputConstants;
import java.util.function.Consumer;
import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
import net.fabricmc.fabric.api.client.rendering.v1.HudRenderCallback;
import net.minecraft.client.KeyMapping;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.client.gui.@OPTIONS_PACKAGE@.OptionsScreen;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.network.chat.Component;

final class ClientPlatform {
    static KeyMapping registerMenuKey() {
        return KeyBindingHelper.registerKeyBinding(new KeyMapping("key.rocket_companion.menu",
            InputConstants.Type.KEYSYM, InputConstants.UNKNOWN.getValue(), "key.categories.rocket_companion.client"));
    }

    @SuppressWarnings("deprecation")
    static void registerHud(Consumer<GuiGraphics> render) {
        HudRenderCallback.EVENT.register((graphics, delta) -> render.accept(graphics));
    }

    static boolean debugVisible(Minecraft client) { return @DEBUG_VISIBLE@; }

    static void registerCrosshair() {} // LegacyCrosshairMixin hooks the older GUI.

    static int fps(Minecraft client) { return @FPS@; }

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
        public boolean keyPressed(int key, int scanCode, int modifiers) {
            return moveSelection(key) || super.keyPressed(key, scanCode, modifiers);
        }

        @Override
        public boolean mouseClicked(double x, double y, int button) {
            return grab(x, y, button) || super.mouseClicked(x, y, button);
        }

        @Override
        public boolean mouseDragged(double x, double y, int button, double dx, double dy) {
            return drag(x, y, button) || super.mouseDragged(x, y, button, dx, dy);
        }

        @Override
        public boolean mouseReleased(double x, double y, int button) {
            return drop(button) || super.mouseReleased(x, y, button);
        }
    }
}

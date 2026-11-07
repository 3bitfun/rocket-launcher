package com.rocketlauncher.companion;

import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.screen.v1.ScreenEvents;
import net.fabricmc.fabric.api.client.screen.v1.Screens;
import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.screens.PauseScreen;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.TitleScreen;
import net.minecraft.client.gui.screens.options.VideoSettingsScreen;
import net.minecraft.client.gui.screens.options.controls.ControlsScreen;
import net.minecraft.network.chat.Component;

public final class RocketCompanion implements ClientModInitializer {
    @Override
    public void onInitializeClient() {
        ScreenEvents.AFTER_INIT.register((client, screen, width, height) -> {
            if (screen instanceof TitleScreen || screen instanceof PauseScreen) {
                Screens.getButtons(screen).add(
                    Button.builder(Component.translatable("rocket_companion.button"),
                        button -> client.setScreen(new RocketMenu(screen)))
                        .bounds(8, 8, 72, 20)
                        .build()
                );
            }
        });
    }

    private static final class RocketMenu extends Screen {
        private final Screen parent;

        private RocketMenu(Screen parent) {
            super(Component.translatable("rocket_companion.title"));
            this.parent = parent;
        }

        @Override
        protected void init() {
            int x = width / 2 - 92;
            int y = height / 2 - 19;
            addRenderableWidget(Button.builder(Component.translatable("rocket_companion.video"),
                button -> minecraft.setScreen(new VideoSettingsScreen(this, minecraft, minecraft.options)))
                .bounds(x, y, 184, 20).build());
            addRenderableWidget(Button.builder(Component.translatable("rocket_companion.controls"),
                button -> minecraft.setScreen(new ControlsScreen(this, minecraft.options)))
                .bounds(x, y + 25, 184, 20).build());
            addRenderableWidget(Button.builder(Component.translatable("rocket_companion.back"),
                button -> onClose()).bounds(x, y + 58, 184, 20).build());
        }

        @Override
        public void render(GuiGraphics graphics, int mouseX, int mouseY, float delta) {
            super.render(graphics, mouseX, mouseY, delta);
            graphics.drawCenteredString(font, title, width / 2, height / 2 - 58, 0xFFFF896A);
            graphics.drawCenteredString(font, Component.translatable("rocket_companion.subtitle"),
                width / 2, height / 2 - 42, 0xFFE0E6EF);
        }

        @Override
        public void onClose() {
            minecraft.setScreen(parent);
        }
    }
}

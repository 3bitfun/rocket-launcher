package com.rocketlauncher.companion.mixin;

import com.rocketlauncher.companion.CrosshairRenderer;
import net.minecraft.client.gui.Gui;
import net.minecraft.client.gui.GuiGraphics;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(Gui.class)
abstract class LegacyCrosshairMixin {
    @Inject(method = "renderCrosshair", at = @At("HEAD"), cancellable = true, require = 0)
    private void rocketCompanionCrosshair(GuiGraphics graphics, @CROSSHAIR_TICK@CallbackInfo callback) {
        if (CrosshairRenderer.renderInGame(graphics)) callback.cancel();
    }
}

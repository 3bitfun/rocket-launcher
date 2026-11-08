package com.rocketlauncher.companion;

import static com.rocketlauncher.companion.RocketCompanion.text;
import static com.rocketlauncher.companion.RocketStyle.*;

import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.network.chat.Component;

final class RocketMenu extends Screen {
    private enum Tab {
        HUD("nav.hud"), MODULES("nav.modules"), SETTINGS("nav.settings");

        final String label;
        Tab(String label) { this.label = label; }
    }

    private static final Tab[] TABS = Tab.values();
    private static final HudModule[] MODULES = HudModule.values();
    private static final int ROW_HEIGHT = 32;
    private static final int BACK_WIDTH = 54;

    private final Screen parent;
    private final ClientSettings settings;
    private final Control[] tabs = new Control[TABS.length];
    private final Control[] toggles = new Control[MODULES.length];
    private Tab tab = Tab.HUD;
    private int page;
    private int pageSize;
    private int lastPage;
    private int left;
    private int top;
    private int span;
    private int panelHeight;
    private int contentY;
    private int footerY;
    private Control previous;
    private Control next;
    private Runnable navigation;

    RocketMenu(Screen parent, ClientSettings settings) {
        super(text("title"));
        this.parent = parent;
        this.settings = settings;
    }

    @Override
    protected void init() {
        settings.refresh();
        span = Math.min(440, width - 16);
        int availableHeight = Math.min(height - 12, 354);
        panelHeight = tab == Tab.MODULES ? availableHeight : Math.min(availableHeight, 168);
        left = (width - span) / 2;
        top = (height - availableHeight) / 2;
        contentY = top + 64;
        footerY = top + panelHeight - 26;
        pageSize = Math.max(1, Math.min(MODULES.length, (footerY - contentY - 3) / ROW_HEIGHT));
        lastPage = (MODULES.length - 1) / pageSize;
        page = Math.min(page, lastPage);

        int tabWidth = (span - 24) / TABS.length;
        for (Tab item : TABS) {
            Component label = text(item.label);
            Control control = new Control(label, pressed -> {
                if (tab != item) navigation = () -> {
                    tab = item;
                    rebuild();
                    focus(tabs[tab.ordinal()]);
                };
            }, left + 8 + item.ordinal() * (tabWidth + 4), top + 37, tabWidth);
            control.state(label, label, item == tab);
            tabs[item.ordinal()] = addRenderableWidget(control);
        }
        addRenderableWidget(new Control(text("back"), pressed -> onClose(),
            left + span - BACK_WIDTH - 6, top + 6, BACK_WIDTH));

        switch (tab) {
            case HUD -> addRenderableWidget(new Control(text("layout"), pressed -> openLayout(null),
                left + 15, overviewActionY(), Math.min(span - 30, 154)));
            case MODULES -> initModules();
            case SETTINGS -> {
                int actionWidth = Math.min((span - 36) / 2, 154);
                addRenderableWidget(new Control(text("crosshair.title"),
                    pressed -> minecraft.setScreen(new CrosshairScreen(this, settings)),
                    left + 15, overviewActionY(), actionWidth));
                addRenderableWidget(new Control(text("options"),
                    pressed -> ClientPlatform.openOptions(this, minecraft),
                    left + 21 + actionWidth, overviewActionY(), actionWidth));
            }
        }
    }

    private int overviewActionY() {
        return contentY + (panelHeight < 168 ? 34 : 47);
    }

    private void initModules() {
        int controlWidth = span >= 300 ? 54 : 42;
        int moveX = left + span - 13 - controlWidth;
        int toggleX = moveX - controlWidth - 4;
        for (int i = page * pageSize; i < Math.min(MODULES.length, (page + 1) * pageSize); i++) {
            HudModule module = MODULES[i];
            int y = rowY(i) + 5;
            Control toggle = new Control(module.displayName(), pressed -> {
                settings.setEnabled(module, !settings.enabled(module));
                updateToggle(module);
            }, toggleX, y, controlWidth);
            toggles[i] = addRenderableWidget(toggle);
            updateToggle(module);

            Component move = text("module.move");
            Control position = new Control(move, pressed -> openLayout(module), moveX, y, controlWidth);
            position.state(text("module.move_named", module.displayName()), move, false);
            addRenderableWidget(position);
        }
        if (lastPage > 0) {
            previous = addRenderableWidget(new Control(text("page.previous"), pressed -> turnPage(-1),
                left + 8, footerY, 65));
            next = addRenderableWidget(new Control(text("page.next"), pressed -> turnPage(1),
                left + span - 73, footerY, 65));
            previous.active = page > 0;
            next.active = page < lastPage;
        }
    }

    private void openLayout(HudModule module) {
        minecraft.setScreen(new HudLayoutScreen(this, settings, module));
    }

    private int rowY(int index) {
        return contentY + (index - page * pageSize) * ROW_HEIGHT;
    }

    private void updateToggle(HudModule module) {
        boolean enabled = settings.enabled(module);
        toggles[module.ordinal()].state(text(enabled ? "module.on" : "module.off", module.displayName()),
            text(enabled ? "state.on" : "state.off"), enabled);
    }

    private void turnPage(int direction) {
        navigation = () -> {
            page = Math.max(0, Math.min(lastPage, page + direction));
            rebuild();
            if (lastPage == 0) {
                focus(tabs[tab.ordinal()]);
                return;
            }
            Control target = direction < 0 ? previous : next;
            Control other = direction < 0 ? next : previous;
            focus(target.active ? target : other);
        };
    }

    private void rebuild() {
        setFocused(null);
        clearWidgets();
        init();
    }

    private void focus(Control target) {
        setFocused(target);
        target.focusForNavigation(true);
    }

    @Override
    public void tick() {
        // Replace controls after the native button has finished dispatching its input event.
        if (navigation != null) {
            Runnable change = navigation;
            navigation = null;
            change.run();
        }
        if (tab == Tab.MODULES) {
            for (int i = page * pageSize; i < Math.min(MODULES.length, (page + 1) * pageSize); i++) {
                updateToggle(MODULES[i]);
            }
        }
    }

    @Override
    public void renderBackground(GuiGraphics graphics, int mouseX, int mouseY, float delta) {
        // The screen draws its own backdrop in render.
    }

    @Override
    public void render(GuiGraphics graphics, int mouseX, int mouseY, float delta) {
        graphics.fill(0, 0, width, height, 0xA6090B10);
        graphics.fill(left + 2, top + 3, left + span + 2, top + panelHeight + 3, 0xA0000000);
        frame(graphics, left, top, span, panelHeight, BORDER, PANEL);
        header(graphics, font, title, left, top, span, BACK_WIDTH + 6);
        graphics.fill(left + 8, contentY - 4, left + span - 8, contentY - 3, BORDER);

        switch (tab) {
            case HUD -> {
                int enabled = 0;
                for (HudModule module : MODULES) if (settings.enabled(module)) enabled++;
                drawOverview(graphics, text("hud.summary", enabled, MODULES.length),
                    text("hud.description"), text("hud.hint"));
            }
            case MODULES -> drawModules(graphics);
            case SETTINGS -> drawOverview(graphics, text(settings.shared() ? "settings.shared" : "settings.local"),
                text(settings.shared() ? "scope.shared" : "scope.local"), text("settings.options_hint"));
        }
        if (settings.saveFailed()) {
            int inset = tab == Tab.MODULES && lastPage > 0 ? 77 : 8;
            line(graphics, font, text("save_error"), left + inset, footerY + 6, span - 2 * inset, WARNING);
        }
        super.render(graphics, mouseX, mouseY, delta);
    }

    private void drawOverview(GuiGraphics graphics, Component heading, Component description, Component hint) {
        int cardHeight = overviewActionY() - contentY + 27;
        frame(graphics, left + 8, contentY, span - 16, cardHeight, BORDER, TILE);
        graphics.fill(left + 9, contentY + 1, left + 11, contentY + cardHeight - 1, ACCENT);
        line(graphics, font, heading, left + 15, contentY + 8, span - 30, TEXT);
        line(graphics, font, description, left + 15, contentY + 22, span - 30, MUTED);
        if (panelHeight >= 168) {
            line(graphics, font, hint, left + 15, contentY + 34, span - 30, MUTED);
        }
    }

    private void drawModules(GuiGraphics graphics) {
        int controlWidth = span >= 300 ? 54 : 42;
        int textWidth = span - 38 - 2 * controlWidth;
        for (int i = page * pageSize; i < Math.min(MODULES.length, (page + 1) * pageSize); i++) {
            HudModule module = MODULES[i];
            int y = rowY(i);
            frame(graphics, left + 8, y, span - 16, 30, BORDER, TILE);
            if (settings.enabled(module)) graphics.fill(left + 9, y + 1, left + 11, y + 29, ACCENT);
            line(graphics, font, module.displayName(), left + 15, y + 5, textWidth, TEXT);
            line(graphics, font, text("description." + module.id), left + 15, y + 17, textWidth, MUTED);
        }
        if (lastPage > 0 && !settings.saveFailed()) {
            graphics.drawCenteredString(font, text("page.count", page + 1, lastPage + 1),
                left + span / 2, footerY + 6, MUTED);
        }
    }

    @Override
    public void onClose() { minecraft.setScreen(parent); }
}

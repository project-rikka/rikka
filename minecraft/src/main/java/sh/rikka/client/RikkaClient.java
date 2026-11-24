package sh.rikka.client;

import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.client.Minecraft;
import org.jspecify.annotations.Nullable;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import sh.rikka.client.config.Preferences;
import sh.rikka.client.config.ProfileManager;
import sh.rikka.client.event.Event;
import sh.rikka.client.event.EventBus;
import sh.rikka.client.events.CombatTracker;
import sh.rikka.client.menu.MenuKeys;
import sh.rikka.client.mod.Mod;
import sh.rikka.client.mod.ModKeybinds;
import sh.rikka.client.mod.ModManager;
import sh.rikka.client.mods.BuiltinMods;
import sh.rikka.client.target.TargetTracker;
import sh.rikka.client.ui.HudOverlays;
import sh.rikka.client.ui.RikkaUi;

public final class RikkaClient implements ClientModInitializer {
	public static final String MOD_ID = "rikka-client";
	public static final String NAME = "Rikka Client";
	public static final Logger LOGGER = LoggerFactory.getLogger(MOD_ID);

	private static final EventBus EVENTS = new EventBus(RikkaClient::onListenerFailure);
	private static final TargetTracker TARGETS = new TargetTracker();
	private static volatile @Nullable Services services;

	@Override
	public void onInitializeClient() {
		ModManager mods = new ModManager(EVENTS);
		BuiltinMods.register(mods);
		mods.lock();
		Preferences preferences = new Preferences();
		ProfileManager profiles = ProfileManager.open(FabricLoader.getInstance().getConfigDir().resolve(MOD_ID), mods, preferences, EVENTS);
		EVENTS.subscribe(new ModKeybinds(mods));
		EVENTS.subscribe(new CombatTracker(EVENTS));
		EVENTS.subscribe(TARGETS);
		EVENTS.subscribe(new HudOverlays(mods));
		EVENTS.subscribe(new MenuKeys(preferences));
		services = new Services(mods, preferences, profiles);
		LOGGER.info("{} initialized with profile {}", NAME, profiles.active());
	}

	public static EventBus events() {
		return EVENTS;
	}

	public static TargetTracker targets() {
		return TARGETS;
	}

	public static ModManager mods() {
		return requireServices().mods();
	}

	public static Preferences preferences() {
		return requireServices().preferences();
	}

	public static ProfileManager profiles() {
		return requireServices().profiles();
	}

	public static void shutdown() {
		RikkaUi.shutdown();
		Services current = services;
		if (current != null) {
			current.profiles().close();
		}
	}

	private static Services requireServices() {
		Services current = services;
		if (current == null) {
			throw new IllegalStateException(NAME + " is not initialized");
		}
		return current;
	}

	private static void onListenerFailure(Object owner, Event event, Exception failure) {
		if (!(owner instanceof Mod mod)) {
			throw new IllegalStateException("listener failed on " + event.getClass().getName(), failure);
		}
		LOGGER.error("{} failed on {} and is disabled", mod.id(), event.getClass().getName(), failure);
		Minecraft minecraft = Minecraft.getInstance();
		if (minecraft.isSameThread()) {
			mod.setEnabled(false);
		} else {
			minecraft.execute(() -> mod.setEnabled(false));
		}
	}

	private record Services(ModManager mods, Preferences preferences, ProfileManager profiles) {
	}
}

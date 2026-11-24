package sh.rikka.client.config;

import java.io.UncheckedIOException;
import java.util.Objects;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.components.toasts.SystemToast;
import net.minecraft.network.chat.Component;
import org.jspecify.annotations.Nullable;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import sh.rikka.client.RikkaClient;
import sh.rikka.client.event.EventHandler;
import sh.rikka.client.events.DisconnectEvent;
import sh.rikka.client.events.ServerConnectEvent;
import sh.rikka.client.events.TickEvent;

public final class ServerProfiles {
	private static final Logger LOGGER = LoggerFactory.getLogger(ServerProfiles.class);
	private static final SystemToast.SystemToastId TOAST = new SystemToast.SystemToastId();
	private static final String KEY = RikkaClient.MOD_ID + ".server_profile.";

	private final ProfileManager profiles;
	private boolean leaving;
	private @Nullable String connectedWith;

	public ServerProfiles(ProfileManager profiles) {
		this.profiles = profiles;
	}

	@EventHandler
	private void onConnect(ServerConnectEvent event) {
		this.leaving = false;
		boolean chosenOnServer = event.transfer() && !this.profiles.active().equals(this.connectedWith);
		if (!chosenOnServer) {
			this.profiles.profileFor(event.host(), event.port()).ifPresentOrElse(this::use, this::restore);
		}
		this.connectedWith = this.profiles.active();
	}

	@EventHandler
	private void onDisconnect(DisconnectEvent event) {
		this.leaving = true;
	}

	@EventHandler
	private void onTick(TickEvent.Pre event) {
		if (this.leaving) {
			this.leaving = false;
			restore();
		}
	}

	private void use(String profile) {
		try {
			if (this.profiles.override(profile)) {
				Notice.SWITCHED.show(profile);
			}
		} catch (UncheckedIOException | IllegalArgumentException failure) {
			failed(failure);
		}
	}

	private void restore() {
		try {
			this.profiles.clearOverride().ifPresent(Notice.RESTORED::show);
		} catch (UncheckedIOException failure) {
			failed(failure);
		}
	}

	private static void failed(RuntimeException failure) {
		LOGGER.error("failed to change to the server profile", failure);
		String reason = Objects.requireNonNullElse(failure.getCause(), failure).getMessage();
		show(Component.translatable(KEY + "failed"), reason == null ? null : Component.literal(reason));
	}

	private static void show(Component title, @Nullable Component message) {
		SystemToast.addOrUpdate(Minecraft.getInstance().gui.toastManager(), TOAST, title, message);
	}

	private enum Notice {
		SWITCHED("switched"),
		RESTORED("restored");

		private final String key;

		Notice(String suffix) {
			this.key = KEY + suffix;
		}

		void show(String profile) {
			ServerProfiles.show(Component.translatable(this.key), Component.translatable(this.key + ".message", profile));
		}
	}
}

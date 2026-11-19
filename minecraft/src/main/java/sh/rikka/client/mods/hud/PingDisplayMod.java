package sh.rikka.client.mods.hud;

import java.util.OptionalLong;
import net.minecraft.client.Minecraft;
import net.minecraft.network.protocol.ping.ClientboundPongResponsePacket;
import sh.rikka.client.event.EventHandler;
import sh.rikka.client.events.PacketEvent;
import sh.rikka.client.events.TickEvent;
import sh.rikka.client.mod.SimpleHudMod;
import sh.rikka.client.net.TabListLatency;
import sh.rikka.client.setting.ScreenPosition;

public final class PingDisplayMod extends SimpleHudMod {
	private final LatencyProbe probe = new LatencyProbe();

	public PingDisplayMod(ScreenPosition defaultPosition) {
		super("ping_display", defaultPosition, "signal_cellular_alt", 8);
	}

	@Override
	protected void onDisable() {
		this.probe.reset();
	}

	@EventHandler
	private void onTick(TickEvent.Post event) {
		this.probe.tick(Minecraft.getInstance().getConnection());
	}

	@EventHandler(receiveCancelled = true)
	private void onReceive(PacketEvent.Receive event) {
		if (event.packet() instanceof ClientboundPongResponsePacket pong) {
			this.probe.receive(pong);
		}
	}

	@Override
	protected String label() {
		OptionalLong measured = this.probe.roundTripMillis();
		if (measured.isPresent()) {
			return translate("format", measured.getAsLong());
		}
		OptionalLong reported = TabListLatency.of(Minecraft.getInstance());
		return reported.isPresent() ? translate("format", reported.getAsLong()) : UNAVAILABLE;
	}
}

package sh.rikka.client.events;

import com.mojang.blaze3d.pipeline.RenderTarget;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.DeltaTracker;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.input.KeyEvent;
import net.minecraft.client.input.MouseButtonInfo;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.network.PacketListener;
import net.minecraft.network.protocol.Packet;
import net.minecraft.network.protocol.PacketFlow;
import net.minecraft.network.protocol.game.ClientGamePacketListener;
import net.minecraft.network.protocol.game.ClientboundBundlePacket;
import sh.rikka.client.RikkaClient;
import sh.rikka.client.event.EventChannel;
import sh.rikka.client.ui.RikkaUi;

public final class ClientHooks {
	private static final EventChannel<TickEvent.Pre> TICK_PRE = RikkaClient.events().channel(TickEvent.Pre.class);
	private static final EventChannel<TickEvent.Post> TICK_POST = RikkaClient.events().channel(TickEvent.Post.class);
	private static final EventChannel<HudRenderEvent> HUD_RENDER = RikkaClient.events().channel(HudRenderEvent.class);
	private static final EventChannel<KeyInputEvent> KEY_INPUT = RikkaClient.events().channel(KeyInputEvent.class);
	private static final EventChannel<MouseInputEvent> MOUSE_INPUT = RikkaClient.events().channel(MouseInputEvent.class);
	private static final EventChannel<MouseScrollEvent> MOUSE_SCROLL = RikkaClient.events().channel(MouseScrollEvent.class);
	private static final EventChannel<PacketEvent.Send> PACKET_SEND = RikkaClient.events().channel(PacketEvent.Send.class);
	private static final EventChannel<PacketEvent.Receive> PACKET_RECEIVE = RikkaClient.events().channel(PacketEvent.Receive.class);
	private static final EventChannel<JumpEvent> JUMP = RikkaClient.events().channel(JumpEvent.class);
	private static final HudRenderEvent HUD_RENDER_EVENT = new HudRenderEvent();

	private ClientHooks() {
	}

	public static void tickPre() {
		TICK_PRE.post(TickEvent.Pre.INSTANCE);
	}

	public static void tickPost() {
		TICK_POST.post(TickEvent.Post.INSTANCE);
	}

	public static void localPlayerJump(LocalPlayer player) {
		if (JUMP.hasListeners()) {
			JUMP.post(new JumpEvent(player));
		}
	}

	public static void hudRender(GuiGraphicsExtractor graphics, DeltaTracker deltaTracker) {
		if (!HUD_RENDER.hasListeners()) {
			return;
		}
		HUD_RENDER_EVENT.update(graphics, deltaTracker);
		try {
			HUD_RENDER.post(HUD_RENDER_EVENT);
		} finally {
			HUD_RENDER_EVENT.clear();
		}
	}

	public static void drawHud(RenderTarget target) {
		RikkaUi.drawHud(target);
	}

	public static void drawScreen(RenderTarget target) {
		RikkaUi.drawScreen(target);
	}

	public static boolean keyInput(KeyEvent input, int action) {
		if (!KEY_INPUT.hasListeners()) {
			return false;
		}
		InputAction inputAction = InputAction.of(action);
		return KEY_INPUT.post(new KeyInputEvent(input, inputAction)).isCancelled() && inputAction != InputAction.RELEASE;
	}

	public static boolean mouseInput(MouseButtonInfo input, int action) {
		if (!MOUSE_INPUT.hasListeners()) {
			return false;
		}
		InputAction inputAction = InputAction.of(action);
		return MOUSE_INPUT.post(new MouseInputEvent(input, inputAction)).isCancelled() && inputAction != InputAction.RELEASE;
	}

	public static boolean mouseScroll(double horizontal, double vertical) {
		return MOUSE_SCROLL.hasListeners() && MOUSE_SCROLL.post(new MouseScrollEvent(horizontal, vertical)).isCancelled();
	}

	public static boolean packetSend(Packet<?> packet, PacketFlow receiving) {
		return receiving == PacketFlow.CLIENTBOUND
			&& PACKET_SEND.hasListeners()
			&& PACKET_SEND.post(new PacketEvent.Send(packet)).isCancelled();
	}

	public static boolean packetReceive(Packet<?> packet, PacketListener listener) {
		if (listener.flow() != PacketFlow.CLIENTBOUND || !PACKET_RECEIVE.hasListeners()) {
			return false;
		}
		if (packet instanceof ClientboundBundlePacket bundle) {
			return bundleReceive(bundle, (ClientGamePacketListener) listener);
		}
		return PACKET_RECEIVE.post(new PacketEvent.Receive(packet)).isCancelled();
	}

	private static boolean bundleReceive(ClientboundBundlePacket bundle, ClientGamePacketListener listener) {
		List<Packet<? super ClientGamePacketListener>> kept = new ArrayList<>();
		boolean cancelled = false;
		for (Packet<? super ClientGamePacketListener> packet : bundle.subPackets()) {
			if (PACKET_RECEIVE.post(new PacketEvent.Receive(packet)).isCancelled()) {
				cancelled = true;
			} else {
				kept.add(packet);
			}
		}
		if (!cancelled) {
			return false;
		}
		new ClientboundBundlePacket(kept).handle(listener);
		return true;
	}
}

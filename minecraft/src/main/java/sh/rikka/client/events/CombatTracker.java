package sh.rikka.client.events;

import java.util.ArrayDeque;
import java.util.Iterator;
import java.util.OptionalInt;
import java.util.OptionalLong;
import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.network.protocol.game.ClientboundDamageEventPacket;
import net.minecraft.network.protocol.game.ServerboundAttackPacket;
import net.minecraft.util.Util;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.boss.enderdragon.EnderDragonPart;
import org.jspecify.annotations.Nullable;
import sh.rikka.client.event.EventBus;
import sh.rikka.client.event.EventChannel;
import sh.rikka.client.event.EventHandler;
import sh.rikka.client.event.Priority;
import sh.rikka.client.net.TabListLatency;

public final class CombatTracker {
	private static final long CONFIRMATION_WINDOW_MILLIS = 3000;
	private static final int NO_ENTITY = -1;

	private final EventChannel<HitEvent> hits;
	private final EventChannel<HurtEvent> hurts;
	private final ArrayDeque<Attack> attacks = new ArrayDeque<>();

	public CombatTracker(EventBus bus) {
		this.hits = bus.channel(HitEvent.class);
		this.hurts = bus.channel(HurtEvent.class);
	}

	@EventHandler(priority = Priority.LOWEST)
	private void onSend(PacketEvent.Send event) {
		if (event.packet() instanceof ServerboundAttackPacket attack && this.hits.hasListeners()) {
			Minecraft.getInstance().execute(() -> recordAttack(attack.entityId()));
		}
	}

	@EventHandler(priority = Priority.LOWEST)
	private void onReceive(PacketEvent.Receive event) {
		if (event.packet() instanceof ClientboundDamageEventPacket damage && (this.hits.hasListeners() || this.hurts.hasListeners())) {
			Minecraft.getInstance().execute(() -> onDamage(damage));
		}
	}

	private void recordAttack(int entityId) {
		Minecraft minecraft = Minecraft.getInstance();
		LocalPlayer player = minecraft.player;
		ClientLevel level = minecraft.level;
		if (player == null || level == null) {
			return;
		}
		Entity struck = struckEntity(level, entityId);
		if (struck == null) {
			return;
		}
		Entity target = struck instanceof EnderDragonPart part ? part.parentMob : struck;
		double reach = Math.sqrt(struck.getBoundingBox().distanceToSqr(player.getEyePosition()));
		long now = Util.getMillis();
		prune(now);
		this.attacks.addLast(new Attack(target.getId(), reach, now));
	}

	private void onDamage(ClientboundDamageEventPacket damage) {
		Minecraft minecraft = Minecraft.getInstance();
		LocalPlayer player = minecraft.player;
		if (player == null) {
			return;
		}
		if (damage.entityId() == player.getId()) {
			int cause = damage.sourceCauseId();
			this.hurts.post(new HurtEvent(player, cause == NO_ENTITY ? OptionalInt.empty() : OptionalInt.of(cause)));
			return;
		}
		if (damage.sourceCauseId() == player.getId() && damage.sourceDirectId() == player.getId()) {
			confirm(damage.entityId(), TabListLatency.of(minecraft));
		}
	}

	private void confirm(int targetId, OptionalLong latency) {
		long now = Util.getMillis();
		prune(now);
		@Nullable Attack confirmed = latency.isPresent() ? closestToAge(targetId, now, latency.getAsLong()) : newest(targetId);
		if (confirmed == null) {
			return;
		}
		Iterator<Attack> iterator = this.attacks.iterator();
		while (iterator.hasNext()) {
			Attack attack = iterator.next();
			if (attack.targetId() == targetId && attack.sentAt() <= confirmed.sentAt()) {
				iterator.remove();
			}
		}
		this.hits.post(new HitEvent(targetId, confirmed.reach()));
	}

	private @Nullable Attack closestToAge(int targetId, long now, long age) {
		@Nullable Attack closest = null;
		long closestDistance = Long.MAX_VALUE;
		for (Attack attack : this.attacks) {
			long distance = Math.abs(now - attack.sentAt() - age);
			if (attack.targetId() == targetId && distance < closestDistance) {
				closest = attack;
				closestDistance = distance;
			}
		}
		return closest;
	}

	private @Nullable Attack newest(int targetId) {
		Iterator<Attack> iterator = this.attacks.descendingIterator();
		while (iterator.hasNext()) {
			Attack attack = iterator.next();
			if (attack.targetId() == targetId) {
				return attack;
			}
		}
		return null;
	}

	private void prune(long now) {
		while (!this.attacks.isEmpty() && now - this.attacks.peekFirst().sentAt() > CONFIRMATION_WINDOW_MILLIS) {
			this.attacks.removeFirst();
		}
	}

	private static @Nullable Entity struckEntity(ClientLevel level, int entityId) {
		Entity entity = level.getEntity(entityId);
		if (entity != null) {
			return entity;
		}
		for (EnderDragonPart part : level.dragonParts()) {
			if (part.getId() == entityId) {
				return part;
			}
		}
		return null;
	}

	private record Attack(int targetId, double reach, long sentAt) {
	}
}

package sh.rikka.client.target;

import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.network.protocol.game.ServerboundAttackPacket;
import net.minecraft.util.Util;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.boss.enderdragon.EnderDragonPart;
import net.minecraft.world.entity.decoration.ArmorStand;
import org.jspecify.annotations.Nullable;
import sh.rikka.client.event.EventHandler;
import sh.rikka.client.event.Priority;
import sh.rikka.client.events.PacketEvent;
import sh.rikka.client.events.TickEvent;

public final class TargetTracker {
	private static final long RETENTION_MILLIS = 3000;
	private static final double MAX_DISTANCE = 16.0;

	private @Nullable LivingEntity target;
	private long lastEngagedAt;

	public @Nullable LivingEntity target() {
		return this.target;
	}

	@EventHandler(priority = Priority.LOWEST)
	private void onSend(PacketEvent.Send event) {
		if (event.packet() instanceof ServerboundAttackPacket attack) {
			int entityId = attack.entityId();
			Minecraft.getInstance().execute(() -> attacked(entityId));
		}
	}

	@EventHandler
	private void onTick(TickEvent.Post event) {
		Minecraft minecraft = Minecraft.getInstance();
		LocalPlayer player = minecraft.player;
		if (player == null || minecraft.level == null) {
			this.target = null;
			return;
		}
		if (minecraft.crosshairPickEntity instanceof LivingEntity hovered && isEligible(hovered, player)) {
			engage(hovered);
		}
		LivingEntity current = this.target;
		if (current != null && !isRetained(current, player)) {
			this.target = null;
		}
	}

	private void attacked(int entityId) {
		Minecraft minecraft = Minecraft.getInstance();
		LocalPlayer player = minecraft.player;
		ClientLevel level = minecraft.level;
		if (player == null || level == null) {
			return;
		}
		Entity struck = level.getEntity(entityId);
		Entity owner = struck instanceof EnderDragonPart part ? part.parentMob : struck;
		if (owner instanceof LivingEntity living && isEligible(living, player)) {
			engage(living);
		}
	}

	private void engage(LivingEntity entity) {
		this.target = entity;
		this.lastEngagedAt = Util.getMillis();
	}

	private boolean isRetained(LivingEntity entity, LocalPlayer player) {
		return isEligible(entity, player)
			&& entity.level() == player.level()
			&& entity.distanceTo(player) <= MAX_DISTANCE
			&& Util.getMillis() - this.lastEngagedAt <= RETENTION_MILLIS;
	}

	private static boolean isEligible(LivingEntity entity, LocalPlayer player) {
		return entity != player && entity.isAlive() && !entity.isRemoved() && !(entity instanceof ArmorStand) && !entity.isInvisibleTo(player);
	}
}

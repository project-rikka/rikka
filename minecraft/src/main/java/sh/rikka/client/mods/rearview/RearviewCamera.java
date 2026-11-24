package sh.rikka.client.mods.rearview;

import net.minecraft.client.Camera;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.phys.Vec3;

final class RearviewCamera extends Camera {
	void aim(Entity entity, Vec3 position, float yRot, float xRot) {
		setEntity(entity);
		setPosition(position);
		setRotation(yRot, xRot);
	}
}

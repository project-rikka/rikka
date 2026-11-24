package sh.rikka.client.mods.animation;

import net.minecraft.client.resources.model.cuboid.ItemTransform;
import net.minecraft.world.item.ItemDisplayContext;
import org.jspecify.annotations.Nullable;

enum ItemFamily {
	HANDHELD(new Display(0, -90, 55, 0, 4, 0.5F, 0.85F), new Display(0, -90, 25, 1.13F, 3.2F, 1.13F, 0.68F)),
	HANDHELD_ROD(new Display(0, 90, 55, 0, 4, 2.5F, 0.85F), new Display(0, 90, 25, 0, 1.6F, 0.8F, 0.68F)),
	BOW(new Display(-80, 260, -40, -1, -2, 2.5F, 0.9F), new Display(0, -90, 25, 1.13F, 3.2F, 1.13F, 0.68F)),
	GENERATED(new Display(0, 0, 0, 0, 3, 1, 0.55F), new Display(0, -90, 25, 1.13F, 3.2F, 1.13F, 0.68F));

	private static final ItemFamily[] FAMILIES = values();
	private static final float TOLERANCE = 1.0E-3F;
	private static final float TRANSLATION_UNIT = 16.0F;

	private final Display thirdPerson;
	private final Display firstPerson;

	ItemFamily(Display thirdPerson, Display firstPerson) {
		this.thirdPerson = thirdPerson;
		this.firstPerson = firstPerson;
	}

	static @Nullable ItemFamily recognize(ItemDisplayContext context, ItemTransform transform) {
		for (ItemFamily family : FAMILIES) {
			Display display = context.firstPerson() ? family.firstPerson : family.thirdPerson;
			if (display.matches(transform)) {
				return family;
			}
		}
		return null;
	}

	boolean rotatesAround() {
		return this == HANDHELD_ROD;
	}

	private record Display(float rx, float ry, float rz, float tx, float ty, float tz, float scale) {
		boolean matches(ItemTransform transform) {
			return near(transform.rotation().x(), this.rx)
				&& near(transform.rotation().y(), this.ry)
				&& near(transform.rotation().z(), this.rz)
				&& near(transform.translation().x() * TRANSLATION_UNIT, this.tx)
				&& near(transform.translation().y() * TRANSLATION_UNIT, this.ty)
				&& near(transform.translation().z() * TRANSLATION_UNIT, this.tz)
				&& near(transform.scale().x(), this.scale)
				&& near(transform.scale().y(), this.scale)
				&& near(transform.scale().z(), this.scale);
		}

		private static boolean near(float actual, float expected) {
			return Math.abs(actual - expected) < TOLERANCE;
		}
	}
}

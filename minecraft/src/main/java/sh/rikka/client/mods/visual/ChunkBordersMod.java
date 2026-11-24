package sh.rikka.client.mods.visual;

import net.minecraft.client.Camera;
import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.core.SectionPos;
import net.minecraft.gizmos.GizmoProperties;
import net.minecraft.gizmos.GizmoStyle;
import net.minecraft.gizmos.Gizmos;
import net.minecraft.util.Mth;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.phys.AABB;
import net.minecraft.world.phys.Vec3;
import org.jspecify.annotations.Nullable;
import sh.rikka.client.mod.Category;
import sh.rikka.client.mod.Mod;
import sh.rikka.client.setting.BoolSetting;
import sh.rikka.client.setting.ColorSetting;
import sh.rikka.client.setting.DoubleSetting;
import sh.rikka.client.setting.EnumSetting;
import sh.rikka.client.setting.IntSetting;

public final class ChunkBordersMod extends Mod {
	private static final int CHUNK = SectionPos.SECTION_SIZE;
	private static final float GRID_WIDTH = 1.0F;
	private static final float SECTION_WIDTH = 1.5F;
	private static volatile @Nullable ChunkBordersMod active;

	private final IntSetting radius = integer("radius", 1, 0, 4);
	private final EnumSetting<ChunkGrid> grid = choice("grid", ChunkGrid.NORMAL);
	private final IntSetting verticalRange = integer("vertical_range", 32, 8, 128).step(8);
	private final BoolSetting section = bool("section", true);
	private final BoolSetting throughWalls = bool("through_walls", false);
	private final DoubleSetting lineWidth = number("line_width", 3.0, 1.0, 8.0).step(0.5);
	private final ColorSetting borderColor = color("border_color", 0xFF4D6BFF);
	private final ColorSetting gridColor = color("grid_color", 0xB3FFE14D);
	private final ColorSetting neighbourColor = color("neighbour_color", 0x99FF4D4D);

	public ChunkBordersMod() {
		super("chunk_borders", Category.VISUAL, "grid_4x4");
	}

	static @Nullable ChunkBordersMod active() {
		return active;
	}

	@Override
	protected void onEnable() {
		active = this;
	}

	@Override
	protected void onDisable() {
		active = null;
	}

	void emitGizmos(Camera camera) {
		ClientLevel level = Minecraft.getInstance().level;
		Entity entity = camera.entity();
		if (level == null || entity == null) {
			return;
		}
		SectionPos home = SectionPos.of(entity.blockPosition());
		int west = home.minBlockX();
		int north = home.minBlockZ();
		int bottom = level.getMinY();
		int top = level.getMaxY() + 1;
		int eye = Mth.floor(camera.position().y);
		int low = Math.max(bottom, eye - this.verticalRange.get());
		int high = Math.min(top, eye + this.verticalRange.get());
		Lines lines = new Lines(this.throughWalls.get(), bottom, top);
		float width = this.lineWidth.getFloat();
		neighbours(lines, west, north, width);
		grid(lines, west, north, low, high);
		int border = this.borderColor.argb();
		for (int y = ceilToMultiple(low, CHUNK); y <= high; y += CHUNK) {
			lines.ring(west, y, north, border, width);
		}
		for (int corner = 0; corner < 4; corner++) {
			lines.pillar(west + (corner & 1) * CHUNK, north + (corner >> 1) * CHUNK, border, width);
		}
		if (this.section.get()) {
			int floor = home.minBlockY();
			AABB box = new AABB(west, floor, north, west + CHUNK, floor + CHUNK, north + CHUNK);
			Gizmos.cuboid(box, GizmoStyle.stroke(border, SECTION_WIDTH)).setAlwaysOnTop();
		}
	}

	private void neighbours(Lines lines, int west, int north, float width) {
		int reach = this.radius.get();
		int color = this.neighbourColor.argb();
		for (int x = -reach; x <= reach + 1; x++) {
			for (int z = -reach; z <= reach + 1; z++) {
				boolean ownCorner = (x == 0 || x == 1) && (z == 0 || z == 1);
				if (!ownCorner) {
					lines.pillar(west + x * CHUNK, north + z * CHUNK, color, width);
				}
			}
		}
	}

	private void grid(Lines lines, int west, int north, int low, int high) {
		int spacing = this.grid.get().spacing();
		if (spacing == 0) {
			return;
		}
		int color = this.gridColor.argb();
		for (int offset = spacing; offset < CHUNK; offset += spacing) {
			lines.pillar(west + offset, north, color, GRID_WIDTH);
			lines.pillar(west + offset, north + CHUNK, color, GRID_WIDTH);
			lines.pillar(west, north + offset, color, GRID_WIDTH);
			lines.pillar(west + CHUNK, north + offset, color, GRID_WIDTH);
		}
		for (int y = ceilToMultiple(low, spacing); y <= high; y += spacing) {
			if (Math.floorMod(y, CHUNK) != 0) {
				lines.ring(west, y, north, color, GRID_WIDTH);
			}
		}
	}

	private static int ceilToMultiple(int value, int multiple) {
		return Math.floorDiv(value + multiple - 1, multiple) * multiple;
	}

	private record Lines(boolean onTop, int bottom, int top) {
		void pillar(int x, int z, int color, float width) {
			line(new Vec3(x, this.bottom, z), new Vec3(x, this.top, z), color, width);
		}

		void ring(int west, int y, int north, int color, float width) {
			Vec3 northWest = new Vec3(west, y, north);
			Vec3 northEast = new Vec3(west + CHUNK, y, north);
			Vec3 southEast = new Vec3(west + CHUNK, y, north + CHUNK);
			Vec3 southWest = new Vec3(west, y, north + CHUNK);
			line(northWest, northEast, color, width);
			line(northEast, southEast, color, width);
			line(southEast, southWest, color, width);
			line(southWest, northWest, color, width);
		}

		private void line(Vec3 from, Vec3 to, int color, float width) {
			GizmoProperties line = Gizmos.line(from, to, color, width);
			if (this.onTop) {
				line.setAlwaysOnTop();
			}
		}
	}
}

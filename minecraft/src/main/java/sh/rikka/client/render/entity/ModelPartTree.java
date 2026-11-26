package sh.rikka.client.render.entity;

import net.minecraft.client.model.geom.ModelPart;

public interface ModelPartTree {
	ModelPart.Cube[] rikka$cubes();

	ModelPart[] rikka$children();

	int rikka$boneCount();
}

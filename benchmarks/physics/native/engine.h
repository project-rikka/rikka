#ifndef RIKKA_BENCH_ENGINE_H
#define RIKKA_BENCH_ENGINE_H

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct bench_vec3 {
	float x;
	float y;
	float z;
} bench_vec3;

typedef struct bench_quat {
	float x;
	float y;
	float z;
	float w;
} bench_quat;

typedef struct bench_pose {
	bench_vec3 p;
	bench_quat q;
} bench_pose;

enum {
	BENCH_STATIC = 0,
	BENCH_KINEMATIC = 1,
	BENCH_DYNAMIC = 2,
};

enum {
	BENCH_CUBOID = 0,
	BENCH_SPHERE = 1,
	BENCH_HULL = 2,
	BENCH_MESH = 3,
};

enum {
	BENCH_SPHERICAL = 0,
	BENCH_REVOLUTE = 1,
};

typedef struct bench_shape {
	uint32_t geometry;
	uint64_t key;
	bench_pose pose;
	bench_vec3 half_extents;
	float radius;
	const bench_vec3* points;
	uint32_t point_count;
	const uint32_t* indices;
	uint32_t index_count;
	float volume;
	bench_vec3 centroid;
	float density;
	float friction;
	float restitution;
	float rolling_resistance;
	uint32_t category;
	uint32_t mask;
} bench_shape;

typedef struct bench_body {
	uint32_t motion;
	bench_pose pose;
	bench_vec3 linear_velocity;
	bench_vec3 angular_velocity;
	bool sleep;
	const bench_shape* shapes;
	uint32_t shape_count;
} bench_body;

typedef struct bench_joint {
	uint32_t kind;
	uint32_t body_a;
	uint32_t body_b;
	bench_pose frame_a;
	bench_pose frame_b;
	bool motor;
	float motor_speed;
	float max_motor_torque;
} bench_joint;

typedef struct bench_world_def {
	uint32_t workers;
	uint32_t substeps;
	bool sleeping;
	float sleep_threshold;
	bench_vec3 gravity;
	uint32_t max_bodies;
} bench_world_def;

typedef struct bench_world bench_world;

typedef struct bench_engine {
	bench_world* ( *create )( const bench_world_def* def );
	void ( *destroy )( bench_world* world );
	uint32_t ( *add_body )( bench_world* world, const bench_body* body );
	bool ( *add_joint )( bench_world* world, const bench_joint* joint );
	void ( *prepare )( bench_world* world );
	void ( *move_kinematic )( bench_world* world, uint32_t body, bench_pose target, float time_step );
	void ( *step )( bench_world* world, float time_step );
	bench_pose ( *pose )( const bench_world* world, uint32_t body );
	uint32_t ( *awake_bodies )( const bench_world* world );
} bench_engine;

#define BENCH_INVALID_BODY UINT32_MAX

const bench_engine* box3d_bench_engine( void );
const bench_engine* jolt_bench_engine( void );
const bench_engine* bullet_bench_engine( void );

#ifdef __cplusplus
}
#endif

#endif

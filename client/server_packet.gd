extends RefCounted

# JSON numbers parsed by Godot are floats. Reject IDs and ticks that might alias.
const MAX_EXACT_JSON_INT: int = 9_007_199_254_740_991
const MAX_U32: int = 4_294_967_295
const MAX_F32: float = 3.4028234663852886e38


static func decode(text: String) -> Dictionary:
	var json: JSON = JSON.new()
	if json.parse(text) != OK or not json.data is Dictionary:
		push_warning("Bỏ qua gói tin có JSON không hợp lệ.")
		return {}

	var packet: Dictionary = json.data
	match packet.get("type"):
		"Welcome":
			if (
				_is_uint(packet.get("player_id"))
				and _is_uint(packet.get("tick_rate"), MAX_U32)
				and packet["tick_rate"] > 0
				and packet.get("map") is String
			):
				return packet
		"PlayerJoined":
			if _is_uint(packet.get("id")) and packet.get("name") is String:
				return packet
		"PlayerLeft":
			if _is_uint(packet.get("id")):
				return packet
		"PlayerStats":
			if (
				_is_uint(packet.get("level"))
				and _is_uint(packet.get("exp"))
				and _is_uint(packet.get("max_exp"))
				and _is_int(packet.get("hp"))
				and _is_int(packet.get("max_hp"))
				and _is_int(packet.get("attack_power"))
				and _is_int(packet.get("defense"))
			):
				return {
					"type": "PlayerStats",
					"level": int(packet["level"]),
					"exp": int(packet["exp"]),
					"max_exp": int(packet["max_exp"]),
					"hp": int(packet["hp"]),
					"max_hp": int(packet["max_hp"]),
					"attack_power": int(packet["attack_power"]),
					"defense": int(packet["defense"]),
				}
		"Inventory":
			if packet.get("slots") is Array:
				var slots: Array[Dictionary] = []
				for item: Variant in packet["slots"]:
					if (
						item is Dictionary
						and _is_uint(item.get("slot"), 19)
						and item.get("item_id") is String
						and _is_uint(item.get("count"), 999)
					):
						slots.append({
							"slot": int(item["slot"]),
							"item_id": item["item_id"],
							"count": int(item["count"]),
						})
				return {
					"type": "Inventory",
					"slots": slots,
				}
		"Snapshot":
			if _is_uint(packet.get("tick")) and packet.get("players") is Array:
				var positions: Dictionary = {}
				var player_meta: Dictionary = {}
				for player: Variant in packet["players"]:
					if (
						not player is Dictionary
						or not _is_uint(player.get("id"))
						or not _is_coordinate(player.get("x"))
						or not _is_coordinate(player.get("y"))
						or positions.has(int(player["id"]))
					):
						push_warning("Bỏ qua Snapshot có dữ liệu người chơi không hợp lệ.")
						return {}
					var p_id: int = int(player["id"])
					positions[p_id] = Vector2(player["x"], player["y"])
					player_meta[p_id] = {
						"hp": int(player.get("hp", 100)),
						"max_hp": int(player.get("max_hp", 100)),
						"level": int(player.get("level", 1)),
					}

				var monsters: Array[Dictionary] = []
				if packet.get("monsters") is Array:
					for monster: Variant in packet["monsters"]:
						if (
							not monster is Dictionary
							or not _is_uint(monster.get("id"))
							or not monster.get("type_id") is String
							or not _is_coordinate(monster.get("x"))
							or not _is_coordinate(monster.get("y"))
							or not monster.get("state") is String
							or not _is_int(monster.get("hp"))
							or not _is_int(monster.get("max_hp"))
						):
							push_warning("Bỏ qua quái vật có dữ liệu không hợp lệ.")
							continue
						monsters.append({
							"id": int(monster["id"]),
							"type_id": monster["type_id"],
							"x": float(monster["x"]),
							"y": float(monster["y"]),
							"state": monster["state"],
							"hp": int(monster["hp"]),
							"max_hp": int(monster["max_hp"]),
						})

				var npcs: Array[Dictionary] = []
				if packet.get("npcs") is Array:
					for npc: Variant in packet["npcs"]:
						if (
							not npc is Dictionary
							or not _is_uint(npc.get("id"))
							or not npc.get("type_id") is String
							or not npc.get("name") is String
							or not _is_coordinate(npc.get("x"))
							or not _is_coordinate(npc.get("y"))
							or not npc.get("state") is String
						):
							push_warning("Bỏ qua NPC có dữ liệu không hợp lệ.")
							continue
						npcs.append({
							"id": int(npc["id"]),
							"type_id": npc["type_id"],
							"name": npc["name"],
							"x": float(npc["x"]),
							"y": float(npc["y"]),
							"state": npc["state"],
						})

				var combat_events: Array[Dictionary] = []
				if packet.get("combat_events") is Array:
					for ev: Variant in packet["combat_events"]:
						if (
							ev is Dictionary
							and _is_uint(ev.get("attacker_id"))
							and _is_uint(ev.get("target_id"))
							and _is_int(ev.get("damage"))
							and _is_int(ev.get("target_hp"))
						):
							combat_events.append({
								"attacker_id": int(ev["attacker_id"]),
								"target_id": int(ev["target_id"]),
								"damage": int(ev["damage"]),
								"is_crit": bool(ev.get("is_crit", false)),
								"target_hp": int(ev["target_hp"]),
							})

				return {
					"type": "Snapshot",
					"tick": int(packet["tick"]),
					"positions": positions,
					"player_meta": player_meta,
					"monsters": monsters,
					"npcs": npcs,
					"combat_events": combat_events,
				}
		"Error":
			if packet.get("message") is String:
				return packet

	push_warning("Bỏ qua gói tin có loại hoặc trường không hợp lệ.")
	return {}


static func _is_uint(value: Variant, maximum: int = MAX_EXACT_JSON_INT) -> bool:
	if not (value is float or value is int):
		return false
	return is_finite(value) and value >= 0 and value <= maximum and floor(value) == value


static func _is_int(value: Variant, maximum: int = MAX_EXACT_JSON_INT) -> bool:
	if not (value is float or value is int):
		return false
	return is_finite(value) and absf(value) <= maximum and floor(value) == value


static func _is_coordinate(value: Variant) -> bool:
	if not (value is float or value is int):
		return false
	return is_finite(value) and absf(value) <= MAX_F32

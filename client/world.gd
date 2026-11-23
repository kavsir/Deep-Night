extends Node2D

const MAP_SIZE: Vector2i = Vector2i(1024, 1024)
const TILE_SIZE: int = 32
const TILE_COLORS: Array[Color] = [Color("#3a5144"), Color("#425b4b")]
const LOCAL_COLOR: Color = Color("#f3c979")
const REMOTE_COLOR: Color = Color("#9eddf3")

const GOBLIN_COLOR: Color = Color("#40c057")
const WOLF_COLOR: Color = Color("#e03131")
const NPC_ELDER_COLOR: Color = Color("#748ffc")
const NPC_GUARD_COLOR: Color = Color("#4dabf7")
const HP_BG_COLOR: Color = Color("#343a40")
const HP_FILL_COLOR: Color = Color("#51cf66")

var local_id: int
var tick_rate: int

var _players: Dictionary = {}
var _camera: Camera2D
var _previous_positions: Dictionary = {}
var _latest_positions: Dictionary = {}

var _monsters: Dictionary = {}
var _previous_monster_positions: Dictionary = {}
var _latest_monster_positions: Dictionary = {}

var _npcs: Dictionary = {}
var _previous_npc_positions: Dictionary = {}
var _latest_npc_positions: Dictionary = {}

var _latest_tick: int = -1
var _snapshot_time_us: int
var _snapshot_interval: float = 0.0


func add_player(id: int, player_name: String) -> void:
	if _players.has(id):
		return

	var player: Node2D = Node2D.new()
	player.hide()
	add_child(player)
	_players[id] = player

	var body: Polygon2D = Polygon2D.new()
	body.polygon = PackedVector2Array([
		Vector2(-11, -11), Vector2(11, -11), Vector2(11, 11), Vector2(-11, 11)
	])
	body.color = LOCAL_COLOR if id == local_id else REMOTE_COLOR
	player.add_child(body)

	var name_label: Label = Label.new()
	name_label.text = player_name
	name_label.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	name_label.add_theme_color_override("font_outline_color", Color("#16201b"))
	name_label.add_theme_constant_override("outline_size", 5)
	player.add_child(name_label)
	var label_size: Vector2 = name_label.get_minimum_size()
	name_label.position = Vector2(-label_size.x / 2.0, -16.0 - label_size.y)

	if id == local_id:
		_camera = Camera2D.new()
		_camera.enabled = false
		player.add_child(_camera)

	_update_player(id, player, _interpolation_weight())


func remove_player(id: int) -> void:
	var player: Node2D = _players.get(id)
	if player == null:
		return
	player.hide()
	player.queue_free()
	_players.erase(id)
	_previous_positions.erase(id)
	_latest_positions.erase(id)


func apply_snapshot(tick: int, positions: Dictionary, monsters_data: Array = [], npcs_data: Array = []) -> void:
	if tick <= _latest_tick:
		return

	_snapshot_interval = float(tick - _latest_tick) / tick_rate if _latest_tick >= 0 else 0.0
	_latest_tick = tick
	_snapshot_time_us = Time.get_ticks_usec()

	# 1. Cập nhật vị trí người chơi
	_previous_positions = _latest_positions
	_latest_positions = positions

	# 2. Cập nhật quái vật
	_previous_monster_positions = _latest_monster_positions
	var new_monster_positions: Dictionary = {}
	for monster: Variant in monsters_data:
		if not monster is Dictionary:
			continue
		var m_id: int = int(monster["id"])
		var m_pos: Vector2 = Vector2(float(monster["x"]), float(monster["y"]))
		new_monster_positions[m_id] = m_pos

		if _monsters.has(m_id):
			_update_monster_data(_monsters[m_id], monster)
		else:
			_add_monster(m_id, monster)
			_previous_monster_positions[m_id] = m_pos

	var removed_monsters: Array = []
	for m_id: int in _monsters:
		if not new_monster_positions.has(m_id):
			removed_monsters.append(m_id)
	for m_id: int in removed_monsters:
		_remove_monster(m_id)
	_latest_monster_positions = new_monster_positions

	# 3. Cập nhật NPC
	_previous_npc_positions = _latest_npc_positions
	var new_npc_positions: Dictionary = {}
	for npc: Variant in npcs_data:
		if not npc is Dictionary:
			continue
		var n_id: int = int(npc["id"])
		var n_pos: Vector2 = Vector2(float(npc["x"]), float(npc["y"]))
		new_npc_positions[n_id] = n_pos

		if _npcs.has(n_id):
			_update_npc_data(_npcs[n_id], npc)
		else:
			_add_npc(n_id, npc)
			_previous_npc_positions[n_id] = n_pos

	var removed_npcs: Array = []
	for n_id: int in _npcs:
		if not new_npc_positions.has(n_id):
			removed_npcs.append(n_id)
	for n_id: int in removed_npcs:
		_remove_npc(n_id)
	_latest_npc_positions = new_npc_positions


func _process(_delta: float) -> void:
	var weight: float = _interpolation_weight()
	for id: int in _players:
		_update_player(id, _players[id], weight)
	for id: int in _monsters:
		_update_monster(id, _monsters[id], weight)
	for id: int in _npcs:
		_update_npc(id, _npcs[id], weight)


func _interpolation_weight() -> float:
	if _snapshot_interval <= 0.0:
		return 1.0
	var elapsed: float = (Time.get_ticks_usec() - _snapshot_time_us) / 1_000_000.0
	return clampf(elapsed / _snapshot_interval, 0.0, 1.0)


func _update_player(id: int, player: Node2D, weight: float) -> void:
	if not _latest_positions.has(id):
		return
	var latest: Vector2 = _latest_positions[id]
	var previous: Vector2 = _previous_positions.get(id, latest)
	player.position = previous.lerp(latest, weight)
	if not player.visible:
		player.show()
		if id == local_id:
			_camera.enabled = true


func _add_monster(id: int, data: Dictionary) -> void:
	var monster: Node2D = Node2D.new()
	monster.hide()
	add_child(monster)
	_monsters[id] = monster

	var type_id: String = data.get("type_id", "goblin")
	var body: Polygon2D = Polygon2D.new()
	body.name = "Body"
	if type_id == "wolf":
		body.polygon = PackedVector2Array([
			Vector2(-13, -7), Vector2(13, 0), Vector2(-13, 7), Vector2(-7, 0)
		])
		body.color = WOLF_COLOR
	else:
		body.polygon = PackedVector2Array([
			Vector2(0, -12), Vector2(11, 0), Vector2(0, 12), Vector2(-11, 0)
		])
		body.color = GOBLIN_COLOR
	monster.add_child(body)

	var name_label: Label = Label.new()
	name_label.name = "NameLabel"
	var display_name: String = "Yêu tinh" if type_id == "goblin" else ("Sói xám" if type_id == "wolf" else type_id)
	name_label.text = display_name
	name_label.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	name_label.add_theme_font_size_override("font_size", 12)
	name_label.add_theme_color_override("font_outline_color", Color("#16201b"))
	name_label.add_theme_constant_override("outline_size", 4)
	monster.add_child(name_label)
	var label_size: Vector2 = name_label.get_minimum_size()
	name_label.position = Vector2(-label_size.x / 2.0, -17.0 - label_size.y)

	var state_label: Label = Label.new()
	state_label.name = "StateLabel"
	state_label.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	state_label.add_theme_font_size_override("font_size", 11)
	state_label.add_theme_color_override("font_outline_color", Color("#16201b"))
	state_label.add_theme_constant_override("outline_size", 4)
	monster.add_child(state_label)
	state_label.position = Vector2(-40.0, -32.0 - label_size.y)
	state_label.custom_minimum_size = Vector2(80, 14)

	var hp_bg: Polygon2D = Polygon2D.new()
	hp_bg.name = "HpBg"
	hp_bg.polygon = PackedVector2Array([
		Vector2(-14, -15), Vector2(14, -15), Vector2(14, -11), Vector2(-14, -11)
	])
	hp_bg.color = HP_BG_COLOR
	monster.add_child(hp_bg)

	var hp_fill: Polygon2D = Polygon2D.new()
	hp_fill.name = "HpFill"
	hp_fill.polygon = PackedVector2Array([
		Vector2(-14, -15), Vector2(14, -15), Vector2(14, -11), Vector2(-14, -11)
	])
	hp_fill.color = HP_FILL_COLOR
	monster.add_child(hp_fill)

	_update_monster_data(monster, data)


func _update_monster_data(monster: Node2D, data: Dictionary) -> void:
	var state: String = data.get("state", "Idle")
	var hp: int = data.get("hp", 50)
	var max_hp: int = data.get("max_hp", 50)

	var hp_fill: Polygon2D = monster.get_node_or_null("HpFill")
	if hp_fill != null:
		var ratio: float = clampf(float(hp) / float(maxi(1, max_hp)), 0.0, 1.0)
		var right_x: float = -14.0 + 28.0 * ratio
		hp_fill.polygon = PackedVector2Array([
			Vector2(-14, -15), Vector2(right_x, -15), Vector2(right_x, -11), Vector2(-14, -11)
		])

	var state_label: Label = monster.get_node_or_null("StateLabel")
	if state_label != null:
		match state:
			"Chase":
				state_label.text = "[Đuổi theo]"
				state_label.add_theme_color_override("font_color", Color("#ff922b"))
			"Attack":
				state_label.text = "[Tấn công!]"
				state_label.add_theme_color_override("font_color", Color("#ff6b6b"))
			"Return":
				state_label.text = "[Quay về]"
				state_label.add_theme_color_override("font_color", Color("#20c997"))
			"Dead":
				state_label.text = "[Đã gục]"
				state_label.add_theme_color_override("font_color", Color("#868e96"))
			"Patrol":
				state_label.text = "[Tuần tra]"
				state_label.add_theme_color_override("font_color", Color("#74c0fc"))
			_:
				state_label.text = ""

	var body: Polygon2D = monster.get_node_or_null("Body")
	if body != null:
		if state == "Dead":
			body.modulate = Color(0.4, 0.4, 0.4, 0.5)
		else:
			body.modulate = Color.WHITE


func _update_monster(id: int, monster: Node2D, weight: float) -> void:
	if not _latest_monster_positions.has(id):
		return
	var latest: Vector2 = _latest_monster_positions[id]
	var previous: Vector2 = _previous_monster_positions.get(id, latest)
	monster.position = previous.lerp(latest, weight)
	if not monster.visible:
		monster.show()


func _remove_monster(id: int) -> void:
	var monster: Node2D = _monsters.get(id)
	if monster != null:
		monster.hide()
		monster.queue_free()
	_monsters.erase(id)
	_previous_monster_positions.erase(id)
	_latest_monster_positions.erase(id)


func _add_npc(id: int, data: Dictionary) -> void:
	var npc: Node2D = Node2D.new()
	npc.hide()
	add_child(npc)
	_npcs[id] = npc

	var type_id: String = data.get("type_id", "village_elder")
	var body: Polygon2D = Polygon2D.new()
	body.name = "Body"
	body.polygon = PackedVector2Array([
		Vector2(-10, -11), Vector2(10, -11), Vector2(10, 3), Vector2(0, 13), Vector2(-10, 3)
	])
	body.color = NPC_ELDER_COLOR if type_id == "village_elder" else NPC_GUARD_COLOR
	npc.add_child(body)

	var name_label: Label = Label.new()
	name_label.name = "NameLabel"
	name_label.text = data.get("name", "NPC")
	name_label.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	name_label.add_theme_font_size_override("font_size", 12)
	name_label.add_theme_color_override("font_outline_color", Color("#16201b"))
	name_label.add_theme_constant_override("outline_size", 4)
	npc.add_child(name_label)
	var label_size: Vector2 = name_label.get_minimum_size()
	name_label.position = Vector2(-label_size.x / 2.0, -17.0 - label_size.y)

	var talk_badge: Label = Label.new()
	talk_badge.name = "TalkBadge"
	talk_badge.text = "💬 Trò chuyện"
	talk_badge.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	talk_badge.add_theme_font_size_override("font_size", 11)
	talk_badge.add_theme_color_override("font_color", Color("#ffd43b"))
	talk_badge.add_theme_color_override("font_outline_color", Color("#16201b"))
	talk_badge.add_theme_constant_override("outline_size", 5)
	npc.add_child(talk_badge)
	var badge_size: Vector2 = talk_badge.get_minimum_size()
	talk_badge.position = Vector2(-badge_size.x / 2.0, -32.0 - label_size.y)
	talk_badge.hide()

	_update_npc_data(npc, data)


func _update_npc_data(npc: Node2D, data: Dictionary) -> void:
	var state: String = data.get("state", "Idle")
	var talk_badge: Label = npc.get_node_or_null("TalkBadge")
	if talk_badge != null:
		talk_badge.visible = (state == "Talk")


func _update_npc(id: int, npc: Node2D, weight: float) -> void:
	if not _latest_npc_positions.has(id):
		return
	var latest: Vector2 = _latest_npc_positions[id]
	var previous: Vector2 = _previous_npc_positions.get(id, latest)
	npc.position = previous.lerp(latest, weight)
	if not npc.visible:
		npc.show()


func _remove_npc(id: int) -> void:
	var npc: Node2D = _npcs.get(id)
	if npc != null:
		npc.hide()
		npc.queue_free()
	_npcs.erase(id)
	_previous_npc_positions.erase(id)
	_latest_npc_positions.erase(id)


func _draw() -> void:
	for row: int in range(ceili(float(MAP_SIZE.y) / TILE_SIZE)):
		for column: int in range(ceili(float(MAP_SIZE.x) / TILE_SIZE)):
			var tile: Rect2 = Rect2(column * TILE_SIZE, row * TILE_SIZE, TILE_SIZE, TILE_SIZE)
			draw_rect(tile, TILE_COLORS[(row + column) % 2])


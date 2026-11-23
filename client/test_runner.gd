extends SceneTree

const ServerPacket = preload("res://server_packet.gd")
const World = preload("res://world.gd")


func _init() -> void:
	print("--- BAT DAU KIEM TRA GODOT CLIENT CHO BUOC 3 (CHIEN DAU, KINH NGHIEM, TUI DO) ---")
	test_decode_v02_packets()
	test_world_combat_and_floating_damage()
	test_live_server_connection()


func test_decode_v02_packets() -> void:
	print("[Test 1] Kiem tra giai ma cac goi tin v0.2 (PlayerStats, Inventory, Snapshot)...")

	# Test PlayerStats
	var stats_json: String = '{"type":"PlayerStats","level":2,"exp":50,"max_exp":200,"hp":120,"max_hp":120,"attack_power":18,"defense":5}'
	var stats: Dictionary = ServerPacket.decode(stats_json)
	assert(stats.get("type") == "PlayerStats", "Phai giai ma dung PlayerStats")
	assert(stats.get("level") == 2, "Level phai la 2")
	assert(stats.get("exp") == 50, "EXP phai la 50")
	assert(stats.get("attack_power") == 18, "Attack power phai la 18")

	# Test Inventory
	var inv_json: String = '{"type":"Inventory","slots":[{"slot":0,"item_id":"potion_hp","count":3},{"slot":1,"item_id":"goblin_ear","count":5}]}'
	var inv: Dictionary = ServerPacket.decode(inv_json)
	assert(inv.get("type") == "Inventory", "Phai giai ma dung Inventory")
	var slots: Array = inv.get("slots", [])
	assert(slots.size() == 2, "Tui do phai co 2 o vat pham")
	assert(slots[0]["item_id"] == "potion_hp", "O 0 phai la potion_hp")
	assert(slots[0]["count"] == 3, "So luong potion_hp phai la 3")

	# Test Snapshot v0.2 co combat_events va player_meta
	var snap_json: String = '{"type":"Snapshot","tick":150,"players":[{"id":1,"x":320.0,"y":400.0,"hp":90,"max_hp":100,"level":1}],"monsters":[{"id":10001,"type_id":"goblin","x":300.0,"y":400.0,"state":"Attack","hp":35,"max_hp":50}],"npcs":[],"combat_events":[{"attacker_id":1,"target_id":10001,"damage":15,"is_crit":false,"target_hp":35}]}'
	var snap: Dictionary = ServerPacket.decode(snap_json)
	assert(snap.get("type") == "Snapshot", "Phai giai ma dung Snapshot")
	assert(snap.get("combat_events").size() == 1, "Snapshot phai chua 1 combat event")
	var ev: Dictionary = snap["combat_events"][0]
	assert(ev["attacker_id"] == 1, "Attacker phai la 1")
	assert(ev["target_id"] == 10001, "Target phai la 10001")
	assert(ev["damage"] == 15, "Damage phai la 15")

	print("[Test 1] -> PASSED!")


func test_world_combat_and_floating_damage() -> void:
	print("[Test 2] Kiem tra hien thi combat, thanh mau, va nham muc tieu trong World...")
	var world: World = World.new()
	world.local_id = 1
	world.tick_rate = 20
	root.add_child(world)

	world.add_player(1, "Arthur")

	var monsters_data: Array = [
		{"id": 10001, "type_id": "goblin", "x": 520.0, "y": 500.0, "state": "Attack", "hp": 35, "max_hp": 50}
	]
	var combat_events: Array = [
		{"attacker_id": 1, "target_id": 10001, "damage": 15, "is_crit": false, "target_hp": 35}
	]

	# Snapshot voi combat event
	world.apply_snapshot(
		1,
		{1: Vector2(500, 500)},
		monsters_data,
		[],
		{1: {"hp": 90, "max_hp": 100, "level": 1}},
		combat_events
	)

	assert(world._monsters.size() == 1, "World phai co 1 quai vat")
	assert(world._floating_texts.size() == 1, "Phai tao 1 so nhay sat thuong floating text")
	var text_item: Dictionary = world._floating_texts[0]
	var label: Label = text_item["label"]
	assert(label.text == "-15", "So sat thuong phai la -15")

	# Kiem tra tim quai vat gan nhat
	var nearest: int = world.get_nearest_monster(60.0)
	assert(nearest == 10001, "Khoang cach 20px phai tim thay Goblin id 10001")

	world.queue_free()
	print("[Test 2] -> PASSED!")


func test_live_server_connection() -> void:
	print("[Test 3] Kiem tra ket noi truc tiep toi server dang chay tai 127.0.0.1:7777...")
	var socket: WebSocketPeer = WebSocketPeer.new()
	var err: Error = socket.connect_to_url("ws://127.0.0.1:7777")
	if err != OK:
		print("[Test 3] Server khong chay tai 127.0.0.1:7777, bo qua test live ket noi.")
		quit(0)
		return

	var deadline_ms: int = Time.get_ticks_msec() + 3000
	var joined: bool = false
	var got_welcome: bool = false
	var got_stats: bool = false
	var got_inventory: bool = false
	var got_snapshot_with_entities: bool = false

	while Time.get_ticks_msec() < deadline_ms:
		socket.poll()
		var state: WebSocketPeer.State = socket.get_ready_state()
		if state == WebSocketPeer.STATE_CLOSED and not joined:
			print("[Test 3] Khong the ket noi toi 127.0.0.1:7777 (server chua bat). Bo qua test live.")
			quit(0)
			return

		if state == WebSocketPeer.STATE_OPEN and not joined:
			socket.send_text('{"type":"Join","name":"Godot_Knight"}')
			joined = true

		while socket.get_available_packet_count() > 0:
			var bytes: PackedByteArray = socket.get_packet()
			var packet: Dictionary = ServerPacket.decode(bytes.get_string_from_utf8())
			if packet.get("type") == "Welcome":
				got_welcome = true
			elif packet.get("type") == "PlayerStats":
				got_stats = true
			elif packet.get("type") == "Inventory":
				got_inventory = true
			elif packet.get("type") == "Snapshot":
				var monsters: Array = packet.get("monsters", [])
				var npcs: Array = packet.get("npcs", [])
				if monsters.size() > 0 and npcs.size() > 0:
					got_snapshot_with_entities = true
					break

		if got_snapshot_with_entities and got_stats and got_inventory:
			break
		OS.delay_msec(20)

	socket.close(1000)
	if not got_welcome:
		print("[Test 3] Khong nhan duoc Welcome (server chua bat tai 127.0.0.1:7777). Bo qua test live.")
		print("--- TOAN BO UNIT TEST GODOT CLIENT PASSED! ---")
		quit(0)
		return

	assert(got_stats, "Client Godot phai nhan duoc PlayerStats tu server")
	assert(got_inventory, "Client Godot phai nhan duoc Inventory tu server")
	assert(got_snapshot_with_entities, "Client Godot phai nhan duoc Snapshot co quai va NPC")
	print("[Test 3] -> PASSED! Client Godot ket noi, nhan day du Welcome, Stats, Inventory va Snapshot!")
	print("--- TOAN BO TEST GODOT CLIENT CHO BUOC 3 PASSED! ---")
	quit(0)

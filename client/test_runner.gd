extends SceneTree

const ServerPacket = preload("res://server_packet.gd")
const World = preload("res://world.gd")


func _init() -> void:
	print("--- BAT DAU KIEM TRA GODOT CLIENT CHO BUOC 2 (QUAI VA NPC) ---")
	test_decode_snapshot()
	test_world_entities()
	test_live_server_connection()


func test_decode_snapshot() -> void:
	print("[Test 1] Kiem tra giai ma Snapshot v0.1 co quai va NPC...")
	var raw_json: String = '{"type":"Snapshot","tick":100,"players":[{"id":1,"x":500.0,"y":500.0}],"monsters":[{"id":10001,"type_id":"goblin","x":300.0,"y":400.0,"state":"Chase","hp":45,"max_hp":50}],"npcs":[{"id":20001,"type_id":"village_elder","name":"Trưởng làng","x":520.0,"y":510.0,"state":"Talk"}]}'
	var packet: Dictionary = ServerPacket.decode(raw_json)
	assert(packet.get("type") == "Snapshot", "Phai giai ma dung Snapshot")
	assert(packet.get("tick") == 100, "Tick phai bang 100")
	assert(packet.get("positions").has(1), "Phai co vi tri player 1")

	var monsters: Array = packet.get("monsters", [])
	assert(monsters.size() == 1, "Phai co 1 quai vat")
	assert(monsters[0]["id"] == 10001, "Monster ID phai la 10001")
	assert(monsters[0]["type_id"] == "goblin", "Monster type phai la goblin")
	assert(monsters[0]["state"] == "Chase", "Monster state phai la Chase")
	assert(monsters[0]["hp"] == 45, "Monster hp phai la 45")

	var npcs: Array = packet.get("npcs", [])
	assert(npcs.size() == 1, "Phai co 1 NPC")
	assert(npcs[0]["id"] == 20001, "NPC ID phai la 20001")
	assert(npcs[0]["state"] == "Talk", "NPC state phai la Talk")
	print("[Test 1] -> PASSED!")


func test_world_entities() -> void:
	print("[Test 2] Kiem tra tao va cap nhat thuc the trong World...")
	var world: World = World.new()
	world.local_id = 1
	world.tick_rate = 20
	root.add_child(world)

	var monsters_data: Array = [
		{"id": 10001, "type_id": "goblin", "x": 300.0, "y": 400.0, "state": "Attack", "hp": 50, "max_hp": 50},
		{"id": 10002, "type_id": "wolf", "x": 700.0, "y": 700.0, "state": "Patrol", "hp": 80, "max_hp": 80}
	]
	var npcs_data: Array = [
		{"id": 20001, "type_id": "village_elder", "name": "Trưởng làng", "x": 520.0, "y": 510.0, "state": "Talk"}
	]

	# Snapshot 1
	world.apply_snapshot(1, {1: Vector2(500, 500)}, monsters_data, npcs_data)
	assert(world._monsters.size() == 2, "World phai co 2 quai vat")
	assert(world._npcs.size() == 1, "World phai co 1 NPC")

	var goblin_node: Node2D = world._monsters[10001]
	var goblin_state_label: Label = goblin_node.get_node("StateLabel")
	assert(goblin_state_label.text == "[Tấn công!]", "Goblin phai hien thi [Tấn công!]")

	var elder_node: Node2D = world._npcs[20001]
	var talk_badge: Label = elder_node.get_node("TalkBadge")
	assert(talk_badge.visible == true, "Talk badge phai hien thi khi Talk")

	# Snapshot 2: Elder tro ve Idle, Goblin bi despawn
	var monsters_data_2: Array = [
		{"id": 10002, "type_id": "wolf", "x": 710.0, "y": 700.0, "state": "Patrol", "hp": 80, "max_hp": 80}
	]
	var npcs_data_2: Array = [
		{"id": 20001, "type_id": "village_elder", "name": "Trưởng làng", "x": 520.0, "y": 510.0, "state": "Idle"}
	]
	world.apply_snapshot(2, {1: Vector2(500, 500)}, monsters_data_2, npcs_data_2)
	assert(world._monsters.size() == 1, "Goblin phai bi remove khoi world khi khong co trong snapshot")
	assert(talk_badge.visible == false, "Talk badge phai an khi Idle")

	world.queue_free()
	print("[Test 2] -> PASSED!")


func test_live_server_connection() -> void:
	print("[Test 3] Kiem tra ket noi truc tiep toi server dang chay...")
	var socket: WebSocketPeer = WebSocketPeer.new()
	var err: Error = socket.connect_to_url("ws://127.0.0.1:7777")
	if err != OK:
		print("[Test 3] Server khong chay tai 127.0.0.1:7777, bo qua test live ket noi.")
		quit(0)
		return

	var deadline_ms: int = Time.get_ticks_msec() + 3000
	var joined: bool = false
	var got_welcome: bool = false
	var got_snapshot_with_entities: bool = false

	while Time.get_ticks_msec() < deadline_ms:
		socket.poll()
		var state: WebSocketPeer.State = socket.get_ready_state()
		if state == WebSocketPeer.STATE_OPEN and not joined:
			socket.send_text('{"type":"Join","name":"Godot_Test"}')
			joined = true

		while socket.get_available_packet_count() > 0:
			var bytes: PackedByteArray = socket.get_packet()
			var packet: Dictionary = ServerPacket.decode(bytes.get_string_from_utf8())
			if packet.get("type") == "Welcome":
				got_welcome = true
			elif packet.get("type") == "Snapshot":
				var monsters: Array = packet.get("monsters", [])
				var npcs: Array = packet.get("npcs", [])
				if monsters.size() > 0 and npcs.size() > 0:
					got_snapshot_with_entities = true
					break

		if got_snapshot_with_entities:
			break
		OS.delay_msec(20)

	socket.close(1000)
	assert(got_welcome, "Client Godot phai nhan duoc Welcome tu server")
	assert(got_snapshot_with_entities, "Client Godot phai nhan duoc Snapshot co quai va NPC")
	print("[Test 3] -> PASSED! Client Godot ket noi va nhan du thuc the tu server!")
	print("--- TOAN BO TEST GODOT CLIENT PASSED! ---")
	quit(0)

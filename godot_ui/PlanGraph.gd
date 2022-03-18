extends GraphEdit

var plan

func new_plan(new_plan):
	plan = new_plan
	name = plan["name"]

func _connection_request(from, from_port, to, to_port):
	connect_node(from, from_port, to, to_port)
	
func _on_GraphPopup_id_pressed(id):
	var node = Global.PlanNode.instance()
	node.offset = (get_global_mouse_position() - get_global_position() + scroll_offset) / zoom
	add_child(node)


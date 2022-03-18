extends TabContainer


# Declare member variables here. Examples:
# var a = 2
# var b = "text"


# Called when the node enters the scene tree for the first time.
func _ready():
	pass # Replace with function body.


# Called every frame. 'delta' is the elapsed time since the previous frame.
#func _process(delta):
#	pass


func _on_Tree_open_plan(plan):
	var plan_node = get_parent().get_child(0).get_node("PlanNode")
	plan_node.new_plan(plan)
	
	for i in get_child_count():
		if get_child(i).plan == plan:
			current_tab = i
			return
	
	var plan_graph = load("PlanGraph.tscn").instance()
	plan_graph.new_plan(plan)
	plan_graph.name = plan["name"]
	add_child(plan_graph)


func _on_CloseTabButton_pressed():
	remove_child(get_child(current_tab))

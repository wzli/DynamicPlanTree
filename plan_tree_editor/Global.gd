extends Node

signal update

var schema
var schema_version := 0
var plan_tree := DEFAULT_PLAN.duplicate()

const DEFAULT_PLAN := {
	"name": "default",
	"active": true,
	"run_interval": 0,
	"behaviour": {"DefaultBehaviour": null},
	"transitions": [],
	"plans": []
}


# Called when the node enters the scene tree for the first time.
func _ready():
	plan_tree["name"] = "root"

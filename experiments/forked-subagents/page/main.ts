import { open, close } from "./open"
import { runs } from "./runs"
import { claims } from "./claims"
import { codePart } from "./code"
import { dataflow } from "./dataflow"
import { typesPart } from "./types"
import { concerns } from "./concerns"

document.getElementById("page")!.append(open(), runs(), claims(), codePart(), dataflow(), typesPart(), concerns(), close())

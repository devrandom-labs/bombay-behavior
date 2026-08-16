struct Child;

#[behavior_macros::births]
enum Children {
    Child(Child),
}

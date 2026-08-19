declare namespace Bap {
  namespace numeric {
    type Unit<Name extends string> = number & {
      readonly __bapUnit: (name: Name) => Name;
    };

    function unit<const Name extends string>(name: Name, value: number): Unit<Name>;
    function unitAdd<const Name extends string>(left: Unit<Name>, right: Unit<Name>): Unit<Name>;
    function unitSub<const Name extends string>(left: Unit<Name>, right: Unit<Name>): Unit<Name>;
    function unitMul(left: Unit<"dimensionless">, right: Unit<"dimensionless">): Unit<"dimensionless">;
    function unitDiv(left: Unit<"dimensionless">, right: Unit<"dimensionless">): Unit<"dimensionless">;
  }
}

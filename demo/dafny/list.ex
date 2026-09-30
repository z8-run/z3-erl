# Adapted from Simple_compiler/Compiler.dfy, LinkedList; see readme.md and license.
defmodule :seq do
  use :vex

  @verifier ensures is_boolean(proper(xs))
  @verifier ensures proper(xs) !== true or is_list(xs)
  @verifier decreases term_size(xs)
  defvg proper(xs) do
    case xs do
      [] -> true
      [_ | t] -> proper(t)
      _ -> false
    end
  end

  @verifier requires proper(ys) === true
  @verifier ensures proper(cat([], ys)) === true
  @verifier ensures is_list(cat([], ys))
  @verifier decreases term_size([])
  defvg cat([], ys) do
    proper(ys)
    ys
  end

  @verifier requires proper([h | t]) === true and proper(ys) === true
  @verifier ensures proper(cat([h | t], ys)) === true
  @verifier ensures is_list(cat([h | t], ys))
  @verifier decreases term_size([h | t])
  defvg cat([h | t], ys) do
    unfold proper([h | t])
    zs = cat(t, ys)
    unfold proper([h | zs])
    [h | zs]
  end

  @verifier requires proper(ys) === true
  @verifier ensures append([], ys) === cat([], ys)
  @verifier ensures proper(append([], ys)) === true
  @verifier decreases term_size([])
  defv append([], ys) do
    unfold cat([], ys)
    ys
  end

  @verifier requires proper([h | t]) === true and proper(ys) === true
  @verifier ensures append([h | t], ys) === cat([h | t], ys)
  @verifier ensures proper(append([h | t], ys)) === true
  @verifier decreases term_size([h | t])
  defv append([h | t], ys) do
    unfold proper([h | t])
    unfold cat([h | t], ys)
    zs = append(t, ys)
    unfold proper([h | zs])
    [h | zs]
  end

  @verifier requires proper(xs) === true and proper(ys) === true and proper(zs) === true
  @verifier ensures cat(cat(xs, ys), zs) === cat(xs, cat(ys, zs))
  @verifier decreases term_size(xs)
  defvg assoc(xs, ys, zs) do
    proper(xs)
    proper(ys)
    xy = cat(xs, ys)
    yz = cat(ys, zs)
    cat(xy, zs)
    cat(xs, yz)

    case xs do
      [] ->
        unfold cat([], ys)
        unfold cat([], yz)

      [h | t] ->
        unfold proper([h | t])
        ty = cat(t, ys)
        assoc(t, ys, zs)
        unfold cat([h | t], ys)
        unfold cat([h | ty], zs)
        unfold cat([h | t], yz)
    end
  end
end

import React from 'react';
import { DynamicUIComponent, DynamicUIAction } from '../types';
import { componentLibrary } from './componentLibrary';

interface DynamicUIRendererProps {
  schema: DynamicUIComponent;
  onAction?: (action: DynamicUIAction) => void;
}

const DynamicUIRenderer: React.FC<DynamicUIRendererProps> = ({ schema, onAction }) => {
  if (!schema) {
    return null;
  }

  const { type, props = {}, children, action } = schema;
  const Component = componentLibrary[type];

  if (!Component) {
    return <div className="text-red-500">Error: Unknown component type "{type}"</div>;
  }

  const handleAction = () => {
    if (action && onAction) {
      onAction(action);
    }
  };

  const interactiveProps = action ? { onClick: handleAction } : {};

  return (
    <Component {...props} {...interactiveProps}>
      {children && children.map((child, index) => {
        if (typeof child === 'string') {
          return child;
        }
        return <DynamicUIRenderer key={index} schema={child} onAction={onAction} />;
      })}
    </Component>
  );
};

export default DynamicUIRenderer;
